use crate::{checked_source_revision, AnalyzedProject, GeneratedArtifact};
use jadpo_diagnostics::{Diagnostic, DiagnosticFact, SourceSpan};
use jadpo_syntax::{
    BinaryOperator, Block, CallableDeclaration, Constraint, ConstraintKind, Declaration,
    EnumDeclaration, Expression, FieldDeclaration, FieldInitialiser, HttpMethod, LiteralKind,
    MatchPattern, PersistenceModifier, RecordDeclaration, ReferenceDeleteAction, Statement,
    TestDeclaration, TypeDeclaration, TypeReference,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub fn derive_target(
    project_path: &Path,
    project: &AnalyzedProject,
) -> Result<Vec<GeneratedArtifact>, Diagnostic> {
    let generator = TargetGenerator::new(project_path, project)?;
    let mut outputs = vec![GeneratedArtifact {
        relative_path: "target/app.ts",
        contents: generator.generate(),
    }];
    if !generator.tests.is_empty() {
        outputs.push(GeneratedArtifact {
            relative_path: "target/tests.ts",
            contents: generator.test_entrypoint(),
        });
    }
    if generator.has_entities() {
        outputs.extend([
            GeneratedArtifact {
                relative_path: "target/persistence.ts",
                contents: generator.persistence_target(),
            },
            GeneratedArtifact {
                relative_path: "sql/postgres/schema.sql",
                contents: generator.schema_sql(SqlDialect::Postgres),
            },
            GeneratedArtifact {
                relative_path: "sql/sqlite/schema.sql",
                contents: generator.schema_sql(SqlDialect::Sqlite),
            },
            GeneratedArtifact {
                relative_path: "persistence/entities.json",
                contents: generator.persistence_manifest(),
            },
        ]);
    }
    validate_runtime_dependency_contract(&outputs)?;
    Ok(outputs)
}

fn validate_runtime_dependency_contract(outputs: &[GeneratedArtifact]) -> Result<(), Diagnostic> {
    for output in outputs {
        let path = Path::new(output.relative_path);
        let file_name = path.file_name().and_then(|name| name.to_str());
        if matches!(file_name, Some("package.json" | "bun.lock" | "bun.lockb"))
            || path
                .components()
                .any(|component| component.as_os_str() == "node_modules")
        {
            return Err(Diagnostic::error("JADPO_TARGET_DEPENDENCY_MANIFEST"));
        }

        if path.extension().and_then(|extension| extension.to_str()) != Some("ts") {
            continue;
        }

        for specifier in module_specifiers(&output.contents) {
            if specifier == "bun" || specifier.starts_with("bun:") {
                continue;
            }
            if let Some(relative) = specifier.strip_prefix("./") {
                let dependency = path
                    .parent()
                    .unwrap_or_else(|| Path::new(""))
                    .join(relative);
                if outputs.iter().any(|candidate| {
                    Path::new(candidate.relative_path) == dependency
                        && Path::new(candidate.relative_path)
                            .extension()
                            .and_then(|extension| extension.to_str())
                            == Some("ts")
                }) {
                    continue;
                }
            }
            return Err(Diagnostic::error("JADPO_TARGET_EXTERNAL_MODULE"));
        }
    }
    Ok(())
}

fn module_specifiers(source: &str) -> Vec<String> {
    let mut specifiers = Vec::new();
    for (marker, quote) in [
        (" from \"", '"'),
        (" from '", '\''),
        ("import \"", '"'),
        ("import '", '\''),
        ("import(\"", '"'),
        ("import('", '\''),
        ("import (\"", '"'),
        ("import ('", '\''),
        ("require(\"", '"'),
        ("require('", '\''),
    ] {
        let mut remaining = source;
        while let Some(start) = remaining.find(marker) {
            let value = &remaining[start + marker.len()..];
            if let Some(end) = value.find(quote) {
                specifiers.push(value[..end].to_owned());
                remaining = &value[end + quote.len_utf8()..];
            } else {
                break;
            }
        }
    }
    specifiers
}

#[derive(Clone, Copy)]
enum SqlDialect {
    Postgres,
    Sqlite,
}

struct TargetGenerator<'project> {
    project: &'project AnalyzedProject,
    types: BTreeMap<String, &'project TypeDeclaration>,
    enums: BTreeMap<String, &'project EnumDeclaration>,
    records: BTreeMap<String, &'project RecordDeclaration>,
    failures: BTreeMap<String, &'project jadpo_syntax::FailureDeclaration>,
    callables: BTreeMap<String, &'project CallableDeclaration>,
    tests: Vec<&'project TestDeclaration>,
    source_revision: String,
}

impl<'project> TargetGenerator<'project> {
    fn new(project_path: &Path, project: &'project AnalyzedProject) -> Result<Self, Diagnostic> {
        let mut types = BTreeMap::new();
        let mut enums = BTreeMap::new();
        let mut records = BTreeMap::new();
        let mut failures = BTreeMap::new();
        let mut callables = BTreeMap::new();
        let mut tests = Vec::new();

        for source in &project.syntax.sources {
            for declaration in &source.file.declarations {
                match declaration {
                    Declaration::Type(declaration) => {
                        types.insert(declaration.name.text.clone(), declaration);
                    }
                    Declaration::Enum(declaration) => {
                        enums.insert(declaration.name.text.clone(), declaration);
                    }
                    Declaration::Record(declaration) => {
                        records.insert(declaration.name.text.clone(), declaration);
                    }
                    Declaration::Callable(declaration) => {
                        callables.insert(declaration.name.text.clone(), declaration);
                    }
                    Declaration::Test(declaration) => tests.push(declaration),
                    Declaration::Failure(declaration) => {
                        failures.insert(declaration.name.text.clone(), declaration);
                    }
                    Declaration::Route(route) if !route.public => {
                        let route_name = format!("{} {}", method_name(route.method), route.path);
                        let mut diagnostic = Diagnostic::error("JADPO_TARGET_AUTH_NOT_IMPLEMENTED")
                            .with_fact(DiagnosticFact::Route(route_name.clone()))
                            .with_impact(route_name);
                        diagnostic.primary = Some(SourceSpan {
                            source: source.source_name.clone(),
                            start: route.range.start,
                            end: route.path_range.end,
                        });
                        return Err(diagnostic);
                    }
                    Declaration::Route(_) => {}
                }
            }
        }

        Ok(Self {
            project,
            types,
            enums,
            records,
            failures,
            callables,
            tests,
            source_revision: checked_source_revision(project_path, project),
        })
    }

    fn generate(&self) -> String {
        let mut output = String::new();
        line(&mut output, "// Generated by Jadpo 0.0.1. Do not edit.");
        line(
            &mut output,
            "// Authored source and semantic metadata are the review surfaces.",
        );
        if self.has_persistence_operations() {
            line(
                &mut output,
                "import { persistence as rootPersistence, PersistenceFault } from \"./persistence.ts\";",
            );
            line(&mut output, "const persistence = rootPersistence;");
            line(&mut output, "");
        }
        line(&mut output, "");
        self.runtime_prelude(&mut output);
        self.type_aliases(&mut output);
        self.validators(&mut output);
        self.failure_contracts(&mut output);
        self.callable_implementations(&mut output);
        self.test_implementations(&mut output);
        self.http_handler(&mut output);
        output
    }

    fn source_revision(&self) -> String {
        self.source_revision.clone()
    }

    fn has_entities(&self) -> bool {
        self.records
            .values()
            .any(|record| record.kind == jadpo_syntax::RecordKind::Entity)
    }

    fn has_persistence_operations(&self) -> bool {
        self.callables
            .values()
            .any(|callable| block_contains_persistence(&callable.body))
            || self
                .tests
                .iter()
                .any(|test| block_contains_persistence(&test.body))
    }

    fn callable_is_mutative(&self, name: &str) -> bool {
        callable_is_mutative(name, &self.callables, &mut BTreeSet::new())
    }

    fn entities(&self) -> impl Iterator<Item = (&String, &&RecordDeclaration)> {
        self.records
            .iter()
            .filter(|(_, declaration)| declaration.kind == jadpo_syntax::RecordKind::Entity)
    }

    fn schema_entities(&self) -> Vec<(&String, &&RecordDeclaration)> {
        let entities = self.entities().collect::<Vec<_>>();
        let mut emitted = BTreeSet::new();
        let mut ordered = Vec::new();

        while ordered.len() < entities.len() {
            let next = entities.iter().copied().find(|(name, entity)| {
                !emitted.contains(name.as_str())
                    && entity.fields.iter().all(|field| {
                        field.reference.as_ref().map_or(true, |reference| {
                            let target = &reference.target.path[0].text;
                            target == name.as_str()
                                || !self.records.contains_key(target)
                                || emitted.contains(target.as_str())
                        })
                    })
            });
            let Some((name, entity)) = next else {
                ordered.extend(
                    entities
                        .iter()
                        .copied()
                        .filter(|(name, _)| !emitted.contains(name.as_str())),
                );
                break;
            };
            emitted.insert(name.as_str());
            ordered.push((name, entity));
        }
        ordered
    }

    fn schema_sql(&self, dialect: SqlDialect) -> String {
        let mut output = String::from("-- Generated by Jadpo 0.0.1. Do not edit.\n\n");
        for (name, entity) in self.schema_entities() {
            let table = sql_identifier(&snake_case(name));
            let body = self.table_body(name, entity, dialect, "  ", ",\n");
            line(
                &mut output,
                &format!("CREATE TABLE IF NOT EXISTS {table} (\n{body}\n);"),
            );
            for statement in self.index_statements(name, entity) {
                line(&mut output, &statement);
            }
            line(&mut output, "");
        }
        output
    }

    fn persistence_manifest(&self) -> String {
        let entities = self
            .entities()
            .map(|(name, entity)| {
                let fields = entity
                    .fields
                    .iter()
                    .map(|field| {
                        let reference = field.reference.as_ref().map_or_else(
                            || "null".to_owned(),
                            |reference| {
                                format!(
                                    "{{\"entity\":{},\"field\":{},\"relationship\":{},\"required\":{},\"on_delete\":{}}}",
                                    ts_string(&reference.target.path[0].text),
                                    ts_string(&reference.target.path[1].text),
                                    ts_string(owning_relationship_name(field)),
                                    !field.field_type.nullable,
                                    ts_string(reference_delete_name(reference.on_delete))
                                )
                            },
                        );
                        format!(
                            "{{\"name\":{},\"type\":{},\"nullable\":{},\"identity\":{},\"unique\":{},\"indexed\":{},\"reference\":{reference}}}",
                            ts_string(&field.name.text),
                            ts_string(&type_name(&field.field_type)),
                            field.field_type.nullable,
                            has_modifier(field, PersistenceModifier::Identity),
                            has_modifier(field, PersistenceModifier::Unique),
                            has_modifier(field, PersistenceModifier::Index)
                                || field.reference.is_some()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                let identity = entity
                    .fields
                    .iter()
                    .find(|field| has_modifier(field, PersistenceModifier::Identity))
                    .map(|field| ts_string(&field.name.text))
                    .unwrap_or_else(|| "null".to_owned());
                let unique_constraints = entity
                    .fields
                    .iter()
                    .filter(|field| has_modifier(field, PersistenceModifier::Unique))
                    .map(|field| ts_string(&field.name.text));
                let compound_unique_constraints = entity
                    .persistence_constraints
                    .iter()
                    .map(|constraint| {
                        format!(
                            "{{\"name\":{},\"fields\":[{}]}}",
                            ts_string(&constraint.name.text),
                            constraint
                                .fields
                                .iter()
                                .map(|field| ts_string(&field.text))
                                .collect::<Vec<_>>()
                                .join(",")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                let indexes = entity
                    .fields
                    .iter()
                    .filter(|field| {
                        has_modifier(field, PersistenceModifier::Index)
                            || field.reference.is_some()
                    })
                    .map(|field| ts_string(&field.name.text));
                let inverses = entity
                    .inverses
                    .iter()
                    .map(|inverse| {
                        format!(
                            "{{\"name\":{},\"cardinality\":{},\"entity\":{},\"via\":{}}}",
                            ts_string(&inverse.name.text),
                            ts_string(match inverse.cardinality {
                                jadpo_syntax::InverseCardinality::Many => "many",
                                jadpo_syntax::InverseCardinality::Optional => "optional",
                            }),
                            ts_string(&inverse.target.text),
                            ts_string(&type_name(&inverse.via))
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                let postgres = self.insert_sql(name, entity, SqlDialect::Postgres);
                let sqlite = self.insert_sql(name, entity, SqlDialect::Sqlite);
                let queries = entity
                    .fields
                    .iter()
                    .map(|field| {
                        let postgres = self.select_optional_sql(
                            name,
                            entity,
                            &field.name.text,
                            SqlDialect::Postgres,
                        );
                        let sqlite = self.select_optional_sql(
                            name,
                            entity,
                            &field.name.text,
                            SqlDialect::Sqlite,
                        );
                        let mut entries = vec![format!(
                            "{}:{{\"postgres\":{},\"sqlite\":{}}},{}:{{\"postgres\":{},\"sqlite\":{}}}",
                            ts_string(&format!("optional_by_{}", field.name.text)),
                            ts_string(&postgres),
                            ts_string(&sqlite),
                            ts_string(&format!("required_by_{}", field.name.text)),
                            ts_string(&postgres),
                            ts_string(&sqlite)
                        )];
                        for order in &entity.fields {
                            for (direction, sql_direction) in
                                [("asc", "ASC"), ("desc", "DESC")]
                            {
                                let postgres = self.select_many_sql(
                                    name,
                                    entity,
                                    &field.name.text,
                                    &order.name.text,
                                    sql_direction,
                                    SqlDialect::Postgres,
                                );
                                let sqlite = self.select_many_sql(
                                    name,
                                    entity,
                                    &field.name.text,
                                    &order.name.text,
                                    sql_direction,
                                    SqlDialect::Sqlite,
                                );
                                entries.push(format!(
                                    "{}:{{\"postgres\":{},\"sqlite\":{}}}",
                                    ts_string(&format!(
                                        "many_by_{}_order_by_{}_{}",
                                        field.name.text, order.name.text, direction
                                    )),
                                    ts_string(&postgres),
                                    ts_string(&sqlite)
                                ));
                                let postgres = self.select_many_paginated_sql(
                                    name,
                                    entity,
                                    &field.name.text,
                                    &order.name.text,
                                    sql_direction,
                                    SqlDialect::Postgres,
                                );
                                let sqlite = self.select_many_paginated_sql(
                                    name,
                                    entity,
                                    &field.name.text,
                                    &order.name.text,
                                    sql_direction,
                                    SqlDialect::Sqlite,
                                );
                                entries.push(format!(
                                    "{}:{{\"postgres\":{},\"sqlite\":{}}}",
                                    ts_string(&format!(
                                        "many_by_{}_order_by_{}_{}_paginated",
                                        field.name.text, order.name.text, direction
                                    )),
                                    ts_string(&postgres),
                                    ts_string(&sqlite)
                                ));
                            }
                        }
                        entries.join(",")
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                let table = sql_identifier(&snake_case(name));
                let returned_fields = entity
                    .fields
                    .iter()
                    .map(|field| sql_identifier(&field.name.text))
                    .collect::<Vec<_>>()
                    .join(", ");
                let mut mutations = Vec::new();
                for predicate in &entity.fields {
                    for change in &entity.fields {
                        let key = format!(
                            "update_required_by_{}_set_{}",
                            predicate.name.text, change.name.text
                        );
                        let postgres = format!(
                            "UPDATE {table} SET {} = $1 WHERE {} = $2 RETURNING {returned_fields}",
                            sql_identifier(&change.name.text),
                            sql_identifier(&predicate.name.text)
                        );
                        let sqlite = format!(
                            "UPDATE {table} SET {} = ?1 WHERE {} = ?2 RETURNING {returned_fields}",
                            sql_identifier(&change.name.text),
                            sql_identifier(&predicate.name.text)
                        );
                        mutations.push(format!(
                            "{}:{{\"postgres\":{},\"sqlite\":{}}}",
                            ts_string(&key),
                            ts_string(&postgres),
                            ts_string(&sqlite)
                        ));
                    }
                    let key = format!("delete_required_by_{}", predicate.name.text);
                    let postgres = format!(
                        "DELETE FROM {table} WHERE {} = $1 RETURNING {returned_fields}",
                        sql_identifier(&predicate.name.text)
                    );
                    let sqlite = format!(
                        "DELETE FROM {table} WHERE {} = ?1 RETURNING {returned_fields}",
                        sql_identifier(&predicate.name.text)
                    );
                    mutations.push(format!(
                        "{}:{{\"postgres\":{},\"sqlite\":{}}}",
                        ts_string(&key),
                        ts_string(&postgres),
                        ts_string(&sqlite)
                    ));
                }
                let mut emitted_multi_updates = BTreeSet::new();
                for callable in self.callables.values() {
                    let mut updates = Vec::new();
                    collect_update_expressions(&callable.body, &mut updates);
                    for update in updates.iter().copied().filter(|update| {
                        update.patch.is_none()
                            && update.changes.len() > 1
                            && update
                                .target
                                .path
                                .iter()
                                .map(|part| part.text.as_str())
                                .collect::<Vec<_>>()
                                .join(".")
                                == *name
                    }) {
                        let change_suffix = update
                            .changes
                            .iter()
                            .map(|change| change.name.text.as_str())
                            .collect::<Vec<_>>()
                            .join("_and_");
                        let key = format!(
                            "update_required_by_{}_set_{change_suffix}",
                            update.field.text
                        );
                        if !emitted_multi_updates.insert(key.clone()) {
                            continue;
                        }
                        let postgres_set = update
                            .changes
                            .iter()
                            .enumerate()
                            .map(|(index, change)| {
                                format!(
                                    "{} = ${}",
                                    sql_identifier(&change.name.text),
                                    index + 1
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        let sqlite_set = update
                            .changes
                            .iter()
                            .enumerate()
                            .map(|(index, change)| {
                                format!(
                                    "{} = ?{}",
                                    sql_identifier(&change.name.text),
                                    index + 1
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        let predicate_index = update.changes.len() + 1;
                        let postgres = format!(
                            "UPDATE {table} SET {postgres_set} WHERE {} = ${predicate_index} RETURNING {returned_fields}",
                            sql_identifier(&update.field.text)
                        );
                        let sqlite = format!(
                            "UPDATE {table} SET {sqlite_set} WHERE {} = ?{predicate_index} RETURNING {returned_fields}",
                            sql_identifier(&update.field.text)
                        );
                        mutations.push(format!(
                            "{}:{{\"postgres\":{},\"sqlite\":{}}}",
                            ts_string(&key),
                            ts_string(&postgres),
                            ts_string(&sqlite)
                        ));
                    }
                    for update in updates.iter().copied().filter(|update| {
                        update.patch.is_some()
                            && update
                                .target
                                .path
                                .iter()
                                .map(|part| part.text.as_str())
                                .collect::<Vec<_>>()
                                .join(".")
                                == *name
                    }) {
                        let patch_fields = self
                            .patch_fields_for_update(update)
                            .expect("checked patch input resolves to a record");
                        let suffix = patch_method_suffix(update, &patch_fields);
                        let key = format!(
                            "update_required_by_{}_{suffix}",
                            update.field.text
                        );
                        if !emitted_multi_updates.insert(key.clone()) {
                            continue;
                        }
                        let mut postgres_set = patch_fields
                            .iter()
                            .enumerate()
                            .map(|(index, field)| {
                                let supplied = index * 2 + 1;
                                let value = supplied + 1;
                                let column = sql_identifier(&field.name.text);
                                format!(
                                    "{column} = CASE WHEN ${supplied} THEN ${value} ELSE {column} END"
                                )
                            })
                            .collect::<Vec<_>>();
                        let mut sqlite_set = patch_fields
                            .iter()
                            .enumerate()
                            .map(|(index, field)| {
                                let supplied = index * 2 + 1;
                                let value = supplied + 1;
                                let column = sql_identifier(&field.name.text);
                                format!(
                                    "{column} = CASE WHEN ?{supplied} THEN ?{value} ELSE {column} END"
                                )
                            })
                            .collect::<Vec<_>>();
                        let derived = patch_derived_changes(update);
                        for (index, (change, supplied)) in derived.iter().enumerate() {
                            let value = patch_fields.len() * 2 + index + 1;
                            let column = sql_identifier(&change.name.text);
                            if let Some(supplied) = supplied {
                                let supplied_field = &supplied.path[1].text;
                                let supplied_index = patch_fields
                                    .iter()
                                    .position(|field| field.name.text == *supplied_field)
                                    .expect("checked supplied patch field exists")
                                    * 2
                                    + 1;
                                postgres_set.push(format!(
                                    "{column} = CASE WHEN ${supplied_index} THEN ${value} ELSE {column} END"
                                ));
                                sqlite_set.push(format!(
                                    "{column} = CASE WHEN ?{supplied_index} THEN ?{value} ELSE {column} END"
                                ));
                            } else {
                                postgres_set.push(format!("{column} = ${value}"));
                                sqlite_set.push(format!("{column} = ?{value}"));
                            }
                        }
                        let postgres_set = postgres_set.join(", ");
                        let sqlite_set = sqlite_set.join(", ");
                        let predicate_index = patch_fields.len() * 2 + derived.len() + 1;
                        let postgres = format!(
                            "UPDATE {table} SET {postgres_set} WHERE {} = ${predicate_index} RETURNING {returned_fields}",
                            sql_identifier(&update.field.text)
                        );
                        let sqlite = format!(
                            "UPDATE {table} SET {sqlite_set} WHERE {} = ?{predicate_index} RETURNING {returned_fields}",
                            sql_identifier(&update.field.text)
                        );
                        mutations.push(format!(
                            "{}:{{\"postgres\":{},\"sqlite\":{},\"omission\":\"supplied_flag\"}}",
                            ts_string(&key),
                            ts_string(&postgres),
                            ts_string(&sqlite)
                        ));
                    }
                }
                let mutations = mutations.join(",");
                format!(
                    "{{\"entity\":{},\"table\":{},\"identity\":{identity},\"unique_constraints\":[{}],\"compound_unique_constraints\":[{compound_unique_constraints}],\"indexes\":[{}],\"fields\":[{fields}],\"inverses\":[{inverses}],\"create\":{{\"postgres\":{},\"sqlite\":{}}},\"queries\":{{{queries}}},\"mutations\":{{{mutations}}}}}",
                    ts_string(name),
                    ts_string(&snake_case(name)),
                    unique_constraints.collect::<Vec<_>>().join(","),
                    indexes.collect::<Vec<_>>().join(","),
                    ts_string(&postgres),
                    ts_string(&sqlite)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let mut query_plans = Vec::new();
        for (child_name, child) in self.entities() {
            let Some(child_identity) = child
                .fields
                .iter()
                .find(|field| has_modifier(field, PersistenceModifier::Identity))
            else {
                continue;
            };
            for field in &child.fields {
                let Some(reference) = &field.reference else {
                    continue;
                };
                let Some(parent_name) =
                    reference.target.path.first().map(|part| part.text.as_str())
                else {
                    continue;
                };
                let Some(target_field) =
                    reference.target.path.get(1).map(|part| part.text.as_str())
                else {
                    continue;
                };
                let Some(parent) = self.records.get(parent_name) else {
                    continue;
                };
                let child_sql = self.select_optional_sql(
                    child_name,
                    child,
                    &child_identity.name.text,
                    SqlDialect::Postgres,
                );
                let parent_sql = self.select_optional_sql(
                    parent_name,
                    parent,
                    target_field,
                    SqlDialect::Postgres,
                );
                query_plans.push(format!(
                    "{{\"child\":{},\"relationship\":{},\"parent\":{},\"strategy\":\"bounded_parent_lookup\",\"query_count\":2,\"child_cardinality\":\"required\",\"parent_cardinality\":{},\"foreign_field\":{},\"target_field\":{},\"child_sql\":{},\"parent_sql\":{}}}",
                    ts_string(child_name),
                    ts_string(owning_relationship_name(field)),
                    ts_string(parent_name),
                    ts_string(if field.field_type.nullable { "optional" } else { "required" }),
                    ts_string(&format!("{child_name}.{}", field.name.text)),
                    ts_string(&format!("{parent_name}.{target_field}")),
                    ts_string(&child_sql),
                    ts_string(&parent_sql)
                ));
                if !field.field_type.nullable {
                    for nested in parent.inverses.iter().filter(|inverse| {
                        inverse.cardinality == jadpo_syntax::InverseCardinality::Optional
                    }) {
                        let Some(nested_child) = self.records.get(&nested.target.text) else {
                            continue;
                        };
                        let Some(nested_via_field) =
                            nested.via.path.get(1).map(|part| part.text.as_str())
                        else {
                            continue;
                        };
                        let Some(nested_target_field) = nested_child
                            .fields
                            .iter()
                            .find(|candidate| candidate.name.text == nested_via_field)
                            .and_then(|candidate| candidate.reference.as_ref())
                            .and_then(|reference| reference.target.path.get(1))
                            .map(|part| part.text.as_str())
                        else {
                            continue;
                        };
                        let leaf_sql = self.select_optional_sql(
                            &nested.target.text,
                            nested_child,
                            nested_via_field,
                            SqlDialect::Postgres,
                        );
                        query_plans.push(format!(
                            "{{\"root\":{},\"path\":[{},{}],\"leaf\":{},\"strategy\":\"bounded_nested_lookup\",\"query_count\":3,\"maximum_depth\":2,\"root_cardinality\":\"required\",\"middle_cardinality\":\"required\",\"leaf_cardinality\":\"optional\",\"leaf_target_field\":{},\"root_sql\":{},\"middle_sql\":{},\"leaf_sql\":{}}}",
                            ts_string(child_name),
                            ts_string(owning_relationship_name(field)),
                            ts_string(&nested.name.text),
                            ts_string(&nested.target.text),
                            ts_string(&format!("{parent_name}.{nested_target_field}")),
                            ts_string(&child_sql),
                            ts_string(&parent_sql),
                            ts_string(&leaf_sql)
                        ));
                    }
                }
            }
        }
        for (parent_name, parent) in self.entities() {
            for inverse in &parent.inverses {
                let Some(child) = self.records.get(&inverse.target.text) else {
                    continue;
                };
                let Some(via_field) = inverse.via.path.get(1).map(|part| part.text.as_str()) else {
                    continue;
                };
                let Some(target_field) = child
                    .fields
                    .iter()
                    .find(|field| field.name.text == via_field)
                    .and_then(|field| field.reference.as_ref())
                    .and_then(|reference| reference.target.path.get(1))
                    .map(|name| name.text.as_str())
                else {
                    continue;
                };
                if inverse.cardinality == jadpo_syntax::InverseCardinality::Optional {
                    let Some(parent_identity) = parent
                        .fields
                        .iter()
                        .find(|field| has_modifier(field, PersistenceModifier::Identity))
                    else {
                        continue;
                    };
                    let parent_sql = self.select_optional_sql(
                        parent_name,
                        parent,
                        &parent_identity.name.text,
                        SqlDialect::Postgres,
                    );
                    let child_sql = self.select_optional_sql(
                        &inverse.target.text,
                        child,
                        via_field,
                        SqlDialect::Postgres,
                    );
                    query_plans.push(format!(
                        "{{\"parent\":{},\"relationship\":{},\"child\":{},\"strategy\":\"bounded_optional_inverse\",\"query_count\":2,\"parent_cardinality\":\"required\",\"child_cardinality\":\"optional\",\"foreign_field\":{},\"target_field\":{},\"parent_sql\":{},\"child_sql\":{}}}",
                        ts_string(parent_name),
                        ts_string(&inverse.name.text),
                        ts_string(&inverse.target.text),
                        ts_string(&format!("{}.{via_field}", inverse.target.text)),
                        ts_string(&format!("{parent_name}.{target_field}")),
                        ts_string(&parent_sql),
                        ts_string(&child_sql)
                    ));
                    continue;
                }
                let Some(parent_order) = parent
                    .fields
                    .iter()
                    .find(|field| has_modifier(field, PersistenceModifier::Identity))
                else {
                    continue;
                };
                let Some(child_order) = child
                    .fields
                    .iter()
                    .find(|field| has_modifier(field, PersistenceModifier::Identity))
                else {
                    continue;
                };
                let parent_sql = self.select_optional_sql(
                    parent_name,
                    parent,
                    target_field,
                    SqlDialect::Postgres,
                );
                let child_sql = self.select_many_paginated_sql(
                    &inverse.target.text,
                    child,
                    via_field,
                    &child_order.name.text,
                    "ASC",
                    SqlDialect::Postgres,
                );
                query_plans.push(format!(
                    "{{\"parent\":{},\"relationship\":{},\"strategy\":\"bounded_batch\",\"query_count\":2,\"parent_cardinality\":\"required\",\"child_cardinality\":\"many\",\"parent_sql\":{},\"child_sql\":{},\"order_by\":{},\"direction\":\"asc\",\"pagination\":{{\"limit_parameter\":2,\"offset_parameter\":3}}}}",
                    ts_string(parent_name),
                    ts_string(&inverse.name.text),
                    ts_string(&parent_sql),
                    ts_string(&child_sql),
                    ts_string(&format!("{}.{}", inverse.target.text, child_order.name.text))
                ));

                let parent_predicate = parent
                    .fields
                    .iter()
                    .find(|field| {
                        has_modifier(field, PersistenceModifier::Index)
                            && !has_modifier(field, PersistenceModifier::Identity)
                            && !has_modifier(field, PersistenceModifier::Unique)
                    })
                    .unwrap_or(parent_order);
                let postgres = self.select_many_with_inverse_sql(
                    parent_name,
                    parent,
                    &parent_predicate.name.text,
                    &parent_order.name.text,
                    "ASC",
                    &inverse.target.text,
                    child,
                    via_field,
                    target_field,
                    &child_order.name.text,
                    "ASC",
                    SqlDialect::Postgres,
                );
                let sqlite = self.select_many_with_inverse_sql(
                    parent_name,
                    parent,
                    &parent_predicate.name.text,
                    &parent_order.name.text,
                    "ASC",
                    &inverse.target.text,
                    child,
                    via_field,
                    target_field,
                    &child_order.name.text,
                    "ASC",
                    SqlDialect::Sqlite,
                );
                query_plans.push(format!(
                    "{{\"parent\":{},\"relationship\":{},\"strategy\":\"parent_page_join\",\"query_count\":1,\"parent_cardinality\":\"many\",\"child_cardinality\":\"many\",\"parent_pagination_before_join\":true,\"postgres\":{},\"sqlite\":{},\"parameters\":{{\"predicate\":1,\"parent_limit\":2,\"parent_offset\":3,\"child_limit\":4,\"child_offset\":5}}}}",
                    ts_string(parent_name),
                    ts_string(&inverse.name.text),
                    ts_string(&postgres),
                    ts_string(&sqlite)
                ));
            }
        }
        for callable in self.callables.values() {
            let mut queries = Vec::new();
            collect_query_expressions(&callable.body, &mut queries);
            for query in queries.into_iter().filter(|query| query.includes.len() > 1) {
                let parent = query
                    .target
                    .path
                    .iter()
                    .map(|part| part.text.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                let result = type_name(&query.includes[0].result);
                let relationships = query
                    .includes
                    .iter()
                    .map(|include| ts_string(&include.relationship.text))
                    .collect::<Vec<_>>()
                    .join(",");
                let (cardinality, strategy, query_count) = match query.cardinality {
                    jadpo_syntax::QueryCardinality::Required => (
                        "required",
                        "parent_then_bounded_children",
                        query.includes.len() + 1,
                    ),
                    jadpo_syntax::QueryCardinality::Many => (
                        "many",
                        "independent_parent_page_joins",
                        query.includes.len(),
                    ),
                    jadpo_syntax::QueryCardinality::Optional => continue,
                };
                query_plans.push(format!(
                    "{{\"parent\":{},\"result\":{},\"relationships\":[{relationships}],\"strategy\":{},\"query_count\":{query_count},\"parent_cardinality\":{},\"cartesian_product_avoided\":true}}",
                    ts_string(&parent),
                    ts_string(&result),
                    ts_string(strategy),
                    ts_string(cardinality)
                ));
            }
        }
        let query_plans = query_plans.join(",");
        format!("{{\"schema_version\":1,\"phase\":\"persistence\",\"transaction_policy\":\"mutative_action\",\"nested_transaction_policy\":\"reuse\",\"entities\":[{entities}],\"query_plans\":[{query_plans}]}}\n")
    }

    fn persistence_target(&self) -> String {
        let mut output = String::new();
        line(&mut output, "// Generated by Jadpo 0.0.1. Do not edit.");
        line(&mut output, "import { SQL } from \"bun\";");
        line(&mut output, "import { Database } from \"bun:sqlite\";");
        line(&mut output, "");
        let mut postgres_constraint_ids = Vec::new();
        let mut sqlite_constraint_ids = Vec::new();
        for (name, entity) in self.entities() {
            let table = snake_case(name);
            for field in &entity.fields {
                let logical = format!("{name}.{}", field.name.text);
                if has_modifier(field, PersistenceModifier::Identity) {
                    postgres_constraint_ids.push(format!(
                        "{}:{}",
                        ts_string(&format!("{table}_identity")),
                        ts_string(&logical)
                    ));
                    sqlite_constraint_ids.push(format!(
                        "[{},{}]",
                        ts_string(&format!(
                            "UNIQUE constraint failed: {table}.{}",
                            field.name.text
                        )),
                        ts_string(&logical)
                    ));
                }
                if has_modifier(field, PersistenceModifier::Unique) {
                    postgres_constraint_ids.push(format!(
                        "{}:{}",
                        ts_string(&format!(
                            "{}_{}_unique",
                            table,
                            snake_case(&field.name.text)
                        )),
                        ts_string(&logical)
                    ));
                    sqlite_constraint_ids.push(format!(
                        "[{},{}]",
                        ts_string(&format!(
                            "UNIQUE constraint failed: {table}.{}",
                            field.name.text
                        )),
                        ts_string(&logical)
                    ));
                }
            }
            for constraint in &entity.persistence_constraints {
                let logical = format!("{name}.{}", constraint.name.text);
                postgres_constraint_ids.push(format!(
                    "{}:{}",
                    ts_string(&format!(
                        "{}_{}_unique",
                        table,
                        snake_case(&constraint.name.text)
                    )),
                    ts_string(&logical)
                ));
                sqlite_constraint_ids.push(format!(
                    "[{},{}]",
                    ts_string(&format!(
                        "UNIQUE constraint failed: {}",
                        constraint
                            .fields
                            .iter()
                            .map(|field| format!("{table}.{}", field.text))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )),
                    ts_string(&logical)
                ));
            }
        }
        line(
            &mut output,
            &format!(
                "const postgresConstraintIds: Record<string, string> = {{{}}};",
                postgres_constraint_ids.join(",")
            ),
        );
        line(
            &mut output,
            &format!(
                "const sqliteConstraintIds: Array<[string, string]> = [{}];",
                sqlite_constraint_ids.join(",")
            ),
        );
        line(&mut output, "");
        line(
            &mut output,
            "export type PersistenceFaultKind = \"driver\" | \"constraint\" | \"cardinality\";",
        );
        line(&mut output, "export class PersistenceFault extends Error {");
        line(&mut output, "  readonly operation: string;");
        line(&mut output, "  readonly kind: PersistenceFaultKind;");
        line(&mut output, "  readonly constraint: string | null;");
        line(
            &mut output,
            "  constructor(operation: string, kind: PersistenceFaultKind, cause: unknown, constraint: string | null = null) {",
        );
        line(
            &mut output,
            "    super(`persistence operation failed: ${operation}`, { cause });",
        );
        line(&mut output, "    this.name = \"PersistenceFault\";");
        line(&mut output, "    this.operation = operation;");
        line(&mut output, "    this.kind = kind;");
        line(&mut output, "    this.constraint = constraint;");
        line(&mut output, "  }");
        line(&mut output, "}");
        line(&mut output, "");
        line(
            &mut output,
            "function hasOwn(value: Record<string, unknown>, key: string): boolean {",
        );
        line(
            &mut output,
            "  return Object.prototype.hasOwnProperty.call(value, key);",
        );
        line(&mut output, "}");
        line(&mut output, "");
        line(
            &mut output,
            "function identifyPersistenceConstraint(error: unknown): string | null {",
        );
        line(
            &mut output,
            "  const details = typeof error === \"object\" && error !== null ? error as { constraint?: unknown; message?: unknown } : {};",
        );
        line(
            &mut output,
            "  const postgresName = typeof details.constraint === \"string\" ? details.constraint : \"\";",
        );
        line(
            &mut output,
            "  if (postgresConstraintIds[postgresName] !== undefined) return postgresConstraintIds[postgresName];",
        );
        line(
            &mut output,
            "  const message = typeof details.message === \"string\" ? details.message : \"\";",
        );
        line(
            &mut output,
            "  for (const [signature, identity] of sqliteConstraintIds) if (message.includes(signature)) return identity;",
        );
        line(&mut output, "  return null;");
        line(&mut output, "}");
        line(&mut output, "");
        line(
            &mut output,
            "function classifyPersistenceError(error: unknown): PersistenceFaultKind {",
        );
        line(
            &mut output,
            "  const details = typeof error === \"object\" && error !== null ? error as { code?: unknown; errno?: unknown } : {};",
        );
        line(
            &mut output,
            "  const codes = [details.code, details.errno].map(value => String(value ?? \"\"));",
        );
        line(
            &mut output,
            "  return codes.some(code => code.startsWith(\"23\") || code.startsWith(\"SQLITE_CONSTRAINT\")) ? \"constraint\" : \"driver\";",
        );
        line(&mut output, "}");
        line(&mut output, "");
        line(
            &mut output,
            "function persistenceSync<T>(operation: string, execute: () => T): T {",
        );
        line(&mut output, "  try { return execute(); }");
        line(&mut output, "  catch (error) { throw error instanceof PersistenceFault ? error : new PersistenceFault(operation, classifyPersistenceError(error), error, identifyPersistenceConstraint(error)); }");
        line(&mut output, "}");
        line(&mut output, "");
        line(&mut output, "async function persistenceAsync<T>(operation: string, execute: () => Promise<T>): Promise<T> {");
        line(&mut output, "  try { return await execute(); }");
        line(&mut output, "  catch (error) {");
        line(&mut output, "    throw error instanceof PersistenceFault ? error : new PersistenceFault(operation, classifyPersistenceError(error), error, identifyPersistenceConstraint(error));");
        line(&mut output, "  }");
        line(&mut output, "}");
        line(&mut output, "");
        line(
            &mut output,
            "function reportPersistenceStartupFault(error: unknown): void {",
        );
        line(&mut output, "  console.error(JSON.stringify({");
        line(&mut output, "    schemaVersion: 1,");
        line(&mut output, "    kind: \"operational_log_event\",");
        line(&mut output, "    eventName: \"operation.failed\",");
        line(
            &mut output,
            "    classification: \"RUNTIME_STARTUP_FAILED\",",
        );
        line(&mut output, "    requestId: \"startup\",");
        line(&mut output, "    traceId: null,");
        line(&mut output, "    semanticOperationId: \"runtime:start\",");
        line(
            &mut output,
            &format!(
                "    sourceRevision: {},",
                ts_string(&self.source_revision())
            ),
        );
        line(&mut output, "    attributes: {},");
        line(&mut output, "  }));");
        line(
            &mut output,
            "  if (Bun.env.JADPO_DEBUG_TARGET_STACKS === \"1\") console.error(error);",
        );
        line(&mut output, "}");
        line(&mut output, "");
        line(
            &mut output,
            "const localPath = decodeURIComponent(new URL(\"../local.sqlite\", import.meta.url).pathname);",
        );
        line(&mut output, "let postgres: SQL | null = null;");
        line(&mut output, "let sqlite: Database | null = null;");
        line(&mut output, "try {");
        line(
            &mut output,
            "  postgres = Bun.env.DATABASE_URL ? persistenceSync(\"database.open\", () => new SQL({ url: Bun.env.DATABASE_URL!, prepare: false })) : null;",
        );
        line(
            &mut output,
            "  sqlite = postgres === null ? persistenceSync(\"database.open\", () => new Database(Bun.env.SQLITE_PATH ?? localPath, { create: true, strict: true })) : null;",
        );
        line(
            &mut output,
            "  if (sqlite !== null) persistenceSync(\"database.foreign_keys\", () => sqlite!.exec(\"PRAGMA foreign_keys = ON\"));",
        );
        for (name, entity) in self.schema_entities() {
            let table = sql_identifier(&snake_case(name));
            let postgres_columns = self.table_body(name, entity, SqlDialect::Postgres, "", ", ");
            let sqlite_columns = self.table_body(name, entity, SqlDialect::Sqlite, "", ", ");
            line(
                &mut output,
                &format!(
                    "  if (postgres !== null) await persistenceAsync(\"schema.{name}\", () => postgres!`CREATE TABLE IF NOT EXISTS {table} ({postgres_columns})`);"
                ),
            );
            line(
                &mut output,
                &format!(
                    "  else persistenceSync(\"schema.{name}\", () => sqlite!.exec({}));",
                    ts_string(&format!(
                        "CREATE TABLE IF NOT EXISTS {table} ({sqlite_columns})"
                    ))
                ),
            );
            for statement in self.index_statements(name, entity) {
                line(
                    &mut output,
                    &format!(
                        "  if (postgres !== null) await persistenceAsync(\"schema.{name}.index\", () => postgres!`{statement}`);"
                    ),
                );
                line(
                    &mut output,
                    &format!(
                        "  else persistenceSync(\"schema.{name}.index\", () => sqlite!.exec({}));",
                        ts_string(&statement)
                    ),
                );
            }
        }
        line(&mut output, "} catch (error) {");
        line(&mut output, "  reportPersistenceStartupFault(error);");
        line(&mut output, "  process.exit(1);");
        line(&mut output, "}");
        line(&mut output, "");
        line(
            &mut output,
            "let sqliteTransactionTail: Promise<void> = Promise.resolve();",
        );
        line(
            &mut output,
            "async function serializeSQLiteTransaction<T>(execute: () => Promise<T>): Promise<T> {",
        );
        line(
            &mut output,
            "  const result = sqliteTransactionTail.then(execute, execute);",
        );
        line(
            &mut output,
            "  sqliteTransactionTail = result.then(() => undefined, () => undefined);",
        );
        line(&mut output, "  return result;");
        line(&mut output, "}");
        line(&mut output, "");
        line(&mut output, "function createPersistenceClient(postgres: SQL | null, sqlite: Database | null, transactional = false) {");
        line(&mut output, "const client = {");
        line(
            &mut output,
            "  async transaction<T>(work: (client: any) => Promise<T>): Promise<T> {",
        );
        line(&mut output, "    if (transactional) return work(client);");
        line(&mut output, "    if (postgres !== null) {");
        line(&mut output, "      let callbackFailed = false;");
        line(&mut output, "      let callbackError: unknown;");
        line(&mut output, "      try {");
        line(
            &mut output,
            "        return await postgres.begin(async tx => {",
        );
        line(
            &mut output,
            "          try { return await work(createPersistenceClient(tx as SQL, null, true)); }",
        );
        line(&mut output, "          catch (error) { callbackFailed = true; callbackError = error; throw error; }");
        line(&mut output, "        });");
        line(&mut output, "      } catch (error) {");
        line(
            &mut output,
            "        if (callbackFailed) throw callbackError;",
        );
        line(&mut output, "        throw error instanceof PersistenceFault ? error : new PersistenceFault(\"transaction.action\", classifyPersistenceError(error), error, identifyPersistenceConstraint(error));");
        line(&mut output, "      }");
        line(&mut output, "    }");
        line(
            &mut output,
            "    return serializeSQLiteTransaction(async () => {",
        );
        line(&mut output, "      persistenceSync(\"transaction.begin\", () => sqlite!.exec(\"BEGIN IMMEDIATE\"));");
        line(&mut output, "      try {");
        line(
            &mut output,
            "        const result = await work(createPersistenceClient(null, sqlite, true));",
        );
        line(
            &mut output,
            "        persistenceSync(\"transaction.commit\", () => sqlite!.exec(\"COMMIT\"));",
        );
        line(&mut output, "        return result;");
        line(&mut output, "      } catch (error) {");
        line(&mut output, "        try { sqlite!.exec(\"ROLLBACK\"); } catch (rollbackError) { throw new PersistenceFault(\"transaction.rollback\", classifyPersistenceError(rollbackError), rollbackError, identifyPersistenceConstraint(rollbackError)); }");
        line(&mut output, "        throw error;");
        line(&mut output, "      }");
        line(&mut output, "    });");
        line(&mut output, "  },");
        for (name, entity) in self.entities() {
            let postgres_values = entity
                .fields
                .iter()
                .map(|field| format!("${{value[{0}]}}", ts_string(&field.name.text)))
                .collect::<Vec<_>>()
                .join(", ");
            let table = sql_identifier(&snake_case(name));
            let fields = entity
                .fields
                .iter()
                .map(|field| sql_identifier(&field.name.text))
                .collect::<Vec<_>>()
                .join(", ");
            let sqlite_sql = self.insert_sql(name, entity, SqlDialect::Sqlite);
            let sqlite_values = entity
                .fields
                .iter()
                .map(|field| format!("value[{}]", ts_string(&field.name.text)))
                .collect::<Vec<_>>()
                .join(", ");
            line(
                &mut output,
                &format!(
                    "  async create_{name}(value: Record<string, unknown>): Promise<unknown> {{"
                ),
            );
            line(&mut output, "    if (postgres !== null) {");
            line(
                &mut output,
                &format!(
                    "      const rows = await persistenceAsync(\"create.{name}\", () => postgres`INSERT INTO {table} ({fields}) VALUES ({postgres_values}) RETURNING {fields}`);"
                ),
            );
            line(
                &mut output,
                "      if (rows.length !== 1) throw new PersistenceFault(\"create.cardinality\", \"cardinality\", new Error(\"database create returned an unexpected row count\"));",
            );
            line(&mut output, "      return rows[0];");
            line(&mut output, "    }");
            line(
                &mut output,
                &format!(
                    "    return persistenceSync(\"create.{name}\", () => sqlite!.prepare({}).get({sqlite_values}));",
                    ts_string(&sqlite_sql)
                ),
            );
            line(&mut output, "  },");
            for field in &entity.fields {
                let field_name = &field.name.text;
                let postgres_sql =
                    self.select_optional_sql(name, entity, field_name, SqlDialect::Postgres);
                let sqlite_sql =
                    self.select_optional_sql(name, entity, field_name, SqlDialect::Sqlite);
                let postgres_template = postgres_sql.replace("$1", "${value}");
                line(
                    &mut output,
                    &format!(
                        "  async query_optional_{name}_by_{field_name}(value: unknown): Promise<unknown | null> {{"
                    ),
                );
                line(&mut output, "    const rows = postgres !== null");
                line(
                    &mut output,
                    &format!("      ? await persistenceAsync(\"query.{name}.{field_name}\", () => postgres`{postgres_template}`)"),
                );
                line(
                    &mut output,
                    &format!(
                        "      : persistenceSync(\"query.{name}.{field_name}\", () => sqlite!.prepare({}).all(value));",
                        ts_string(&sqlite_sql)
                    ),
                );
                line(
                    &mut output,
                    "    if (rows.length > 1) throw new PersistenceFault(\"query.cardinality\", \"cardinality\", new Error(\"optional query returned more than one row\"));",
                );
                line(&mut output, "    return rows[0] ?? null;");
                line(&mut output, "  },");
                line(
                    &mut output,
                    &format!(
                        "  async query_required_{name}_by_{field_name}(value: unknown): Promise<unknown | null> {{"
                    ),
                );
                line(
                    &mut output,
                    &format!("    return this.query_optional_{name}_by_{field_name}(value);"),
                );
                line(&mut output, "  },");
                for order in &entity.fields {
                    for (direction, sql_direction) in [("asc", "ASC"), ("desc", "DESC")] {
                        let postgres_sql = self.select_many_sql(
                            name,
                            entity,
                            field_name,
                            &order.name.text,
                            sql_direction,
                            SqlDialect::Postgres,
                        );
                        let sqlite_sql = self.select_many_sql(
                            name,
                            entity,
                            field_name,
                            &order.name.text,
                            sql_direction,
                            SqlDialect::Sqlite,
                        );
                        let postgres_template = postgres_sql.replace("$1", "${value}");
                        line(
                            &mut output,
                            &format!(
                                "  async query_many_{name}_by_{field_name}_order_by_{}_{direction}(value: unknown): Promise<unknown[]> {{",
                                order.name.text
                            ),
                        );
                        line(&mut output, "    return postgres !== null");
                        line(
                            &mut output,
                            &format!(
                                "      ? await persistenceAsync(\"query_many.{name}.{field_name}.{}.{direction}\", () => postgres`{postgres_template}`)",
                                order.name.text
                            ),
                        );
                        line(
                            &mut output,
                            &format!(
                                "      : persistenceSync(\"query_many.{name}.{field_name}.{}.{direction}\", () => sqlite!.prepare({}).all(value));",
                                order.name.text,
                                ts_string(&sqlite_sql)
                            ),
                        );
                        line(&mut output, "  },");

                        let postgres_sql = self.select_many_paginated_sql(
                            name,
                            entity,
                            field_name,
                            &order.name.text,
                            sql_direction,
                            SqlDialect::Postgres,
                        );
                        let sqlite_sql = self.select_many_paginated_sql(
                            name,
                            entity,
                            field_name,
                            &order.name.text,
                            sql_direction,
                            SqlDialect::Sqlite,
                        );
                        let postgres_template = postgres_sql
                            .replace("$1", "${value}")
                            .replace("$2", "${limit}")
                            .replace("$3", "${offset}");
                        line(
                            &mut output,
                            &format!(
                                "  async query_many_{name}_by_{field_name}_order_by_{}_{direction}_paginated(value: unknown, limit: unknown, offset: unknown): Promise<unknown[]> {{",
                                order.name.text
                            ),
                        );
                        line(&mut output, "    return postgres !== null");
                        line(
                            &mut output,
                            &format!(
                                "      ? await persistenceAsync(\"query_many_paginated.{name}.{field_name}.{}.{direction}\", () => postgres`{postgres_template}`)",
                                order.name.text
                            ),
                        );
                        line(
                            &mut output,
                            &format!(
                                "      : persistenceSync(\"query_many_paginated.{name}.{field_name}.{}.{direction}\", () => sqlite!.prepare({}).all(value, limit, offset));",
                                order.name.text,
                                ts_string(&sqlite_sql)
                            ),
                        );
                        line(&mut output, "  },");
                    }
                }
            }
            for predicate in &entity.fields {
                let predicate_name = &predicate.name.text;
                let postgres_select = self
                    .select_optional_sql(name, entity, predicate_name, SqlDialect::Postgres)
                    .replace("$1", "${predicateValue}");
                let sqlite_select =
                    self.select_optional_sql(name, entity, predicate_name, SqlDialect::Sqlite);
                for change in &entity.fields {
                    let change_name = &change.name.text;
                    let postgres_update = format!(
                        "UPDATE {table} SET {} = ${{replacement}} WHERE {} = ${{predicateValue}} RETURNING {fields}",
                        sql_identifier(change_name),
                        sql_identifier(predicate_name)
                    );
                    let sqlite_update = format!(
                        "UPDATE {table} SET {} = ?1 WHERE {} = ?2 RETURNING {fields}",
                        sql_identifier(change_name),
                        sql_identifier(predicate_name)
                    );
                    let operation = format!("update.{name}.{predicate_name}.{change_name}");
                    line(
                        &mut output,
                        &format!(
                            "  async update_required_{name}_by_{predicate_name}_set_{change_name}(predicateValue: unknown, replacement: unknown): Promise<unknown | null> {{"
                        ),
                    );
                    line(&mut output, "    if (postgres !== null) {");
                    line(&mut output, "      const execute = async (tx: SQL) => {");
                    line(
                        &mut output,
                        &format!("        const matches = await tx`{postgres_select} FOR UPDATE`;"),
                    );
                    line(&mut output, "        if (matches.length > 1) throw new PersistenceFault(\"update.cardinality\", \"cardinality\", new Error(\"required update matched more than one row\"));");
                    line(
                        &mut output,
                        "        if (matches.length === 0) return null;",
                    );
                    line(
                        &mut output,
                        &format!("        const rows = await tx`{postgres_update}`;"),
                    );
                    line(&mut output, "        if (rows.length !== 1) throw new PersistenceFault(\"update.cardinality\", \"cardinality\", new Error(\"required update changed an unexpected row count\"));");
                    line(&mut output, "        return rows[0];");
                    line(&mut output, "      };");
                    line(
                        &mut output,
                        &format!(
                            "      return persistenceAsync({}, () => transactional ? execute(postgres) : postgres.begin(tx => execute(tx as SQL)));",
                            ts_string(&operation)
                        ),
                    );
                    line(&mut output, "    }");
                    line(
                        &mut output,
                        "    const execute = (predicateValue: unknown, replacement: unknown) => {",
                    );
                    line(
                        &mut output,
                        &format!(
                            "      const matches = sqlite!.prepare({}).all(predicateValue);",
                            ts_string(&sqlite_select)
                        ),
                    );
                    line(&mut output, "      if (matches.length > 1) throw new PersistenceFault(\"update.cardinality\", \"cardinality\", new Error(\"required update matched more than one row\"));");
                    line(&mut output, "      if (matches.length === 0) return null;");
                    line(
                        &mut output,
                        &format!("      const rows = sqlite!.prepare({}).all(replacement, predicateValue);", ts_string(&sqlite_update)),
                    );
                    line(&mut output, "      if (rows.length !== 1) throw new PersistenceFault(\"update.cardinality\", \"cardinality\", new Error(\"required update changed an unexpected row count\"));");
                    line(&mut output, "      return rows[0];");
                    line(&mut output, "    };");
                    line(&mut output, "    if (transactional) return persistenceSync(\"update.transactional\", () => execute(predicateValue, replacement));");
                    line(
                        &mut output,
                        "    const mutate = sqlite!.transaction(execute);",
                    );
                    line(
                        &mut output,
                        &format!("    return persistenceSync({}, () => mutate(predicateValue, replacement));", ts_string(&operation)),
                    );
                    line(&mut output, "  },");
                }
                let postgres_delete = format!(
                    "DELETE FROM {table} WHERE {} = ${{predicateValue}} RETURNING {fields}",
                    sql_identifier(predicate_name)
                );
                let sqlite_delete = format!(
                    "DELETE FROM {table} WHERE {} = ?1 RETURNING {fields}",
                    sql_identifier(predicate_name)
                );
                let operation = format!("delete.{name}.{predicate_name}");
                line(
                    &mut output,
                    &format!(
                        "  async delete_required_{name}_by_{predicate_name}(predicateValue: unknown): Promise<unknown | null> {{"
                    ),
                );
                line(&mut output, "    if (postgres !== null) {");
                line(&mut output, "      const execute = async (tx: SQL) => {");
                line(
                    &mut output,
                    &format!("        const matches = await tx`{postgres_select} FOR UPDATE`;"),
                );
                line(&mut output, "        if (matches.length > 1) throw new PersistenceFault(\"delete.cardinality\", \"cardinality\", new Error(\"required delete matched more than one row\"));");
                line(
                    &mut output,
                    "        if (matches.length === 0) return null;",
                );
                line(
                    &mut output,
                    &format!("        const rows = await tx`{postgres_delete}`;"),
                );
                line(&mut output, "        if (rows.length !== 1) throw new PersistenceFault(\"delete.cardinality\", \"cardinality\", new Error(\"required delete changed an unexpected row count\"));");
                line(&mut output, "        return rows[0];");
                line(&mut output, "      };");
                line(
                    &mut output,
                    &format!(
                        "      return persistenceAsync({}, () => transactional ? execute(postgres) : postgres.begin(tx => execute(tx as SQL)));",
                        ts_string(&operation)
                    ),
                );
                line(&mut output, "    }");
                line(
                    &mut output,
                    "    const execute = (predicateValue: unknown) => {",
                );
                line(
                    &mut output,
                    &format!(
                        "      const matches = sqlite!.prepare({}).all(predicateValue);",
                        ts_string(&sqlite_select)
                    ),
                );
                line(&mut output, "      if (matches.length > 1) throw new PersistenceFault(\"delete.cardinality\", \"cardinality\", new Error(\"required delete matched more than one row\"));");
                line(&mut output, "      if (matches.length === 0) return null;");
                line(
                    &mut output,
                    &format!(
                        "      const rows = sqlite!.prepare({}).all(predicateValue);",
                        ts_string(&sqlite_delete)
                    ),
                );
                line(&mut output, "      if (rows.length !== 1) throw new PersistenceFault(\"delete.cardinality\", \"cardinality\", new Error(\"required delete changed an unexpected row count\"));");
                line(&mut output, "      return rows[0];");
                line(&mut output, "    };");
                line(&mut output, "    if (transactional) return persistenceSync(\"delete.transactional\", () => execute(predicateValue));");
                line(
                    &mut output,
                    "    const mutate = sqlite!.transaction(execute);",
                );
                line(
                    &mut output,
                    &format!(
                        "    return persistenceSync({}, () => mutate(predicateValue));",
                        ts_string(&operation)
                    ),
                );
                line(&mut output, "  },");
            }
        }
        let mut emitted_patch_updates = BTreeSet::new();
        for callable in self.callables.values() {
            let mut updates = Vec::new();
            collect_update_expressions(&callable.body, &mut updates);
            for update in updates.into_iter().filter(|update| update.patch.is_some()) {
                let name = update
                    .target
                    .path
                    .iter()
                    .map(|part| part.text.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                let patch_fields = self
                    .patch_fields_for_update(update)
                    .expect("checked patch input resolves to a record");
                let suffix = patch_method_suffix(update, &patch_fields);
                let method = format!("update_required_{name}_by_{}_{suffix}", update.field.text);
                if !emitted_patch_updates.insert(method.clone()) {
                    continue;
                }
                let entity = self
                    .records
                    .get(&name)
                    .expect("checked patch target entity exists");
                let table = sql_identifier(&snake_case(&name));
                let returned_fields = entity
                    .fields
                    .iter()
                    .map(|field| sql_identifier(&field.name.text))
                    .collect::<Vec<_>>()
                    .join(", ");
                let postgres_select = self
                    .select_optional_sql(&name, entity, &update.field.text, SqlDialect::Postgres)
                    .replace("$1", "${predicateValue}");
                let sqlite_select =
                    self.select_optional_sql(&name, entity, &update.field.text, SqlDialect::Sqlite);
                let mut postgres_set = patch_fields
                    .iter()
                    .map(|field| {
                        let column = sql_identifier(&field.name.text);
                        let key = ts_string(&field.name.text);
                        format!(
                            "{column} = CASE WHEN ${{hasOwn(patchValue, {key})}} THEN ${{hasOwn(patchValue, {key}) ? patchValue[{key}] : null}} ELSE {column} END"
                        )
                    })
                    .collect::<Vec<_>>();
                let mut sqlite_set = patch_fields
                    .iter()
                    .enumerate()
                    .map(|(index, field)| {
                        let supplied = index * 2 + 1;
                        let value = supplied + 1;
                        let column = sql_identifier(&field.name.text);
                        format!("{column} = CASE WHEN ?{supplied} THEN ?{value} ELSE {column} END")
                    })
                    .collect::<Vec<_>>();
                let derived = patch_derived_changes(update);
                let replacements = (0..derived.len())
                    .map(|index| format!("replacement{index}: unknown"))
                    .collect::<Vec<_>>();
                let replacement_names = (0..derived.len())
                    .map(|index| format!("replacement{index}"))
                    .collect::<Vec<_>>();
                for (index, (change, supplied)) in derived.iter().enumerate() {
                    let column = sql_identifier(&change.name.text);
                    if let Some(supplied) = supplied {
                        let key = ts_string(&supplied.path[1].text);
                        postgres_set.push(format!(
                            "{column} = CASE WHEN ${{hasOwn(patchValue, {key})}} THEN ${{replacement{index}}} ELSE {column} END"
                        ));
                        let supplied_index = patch_fields
                            .iter()
                            .position(|field| field.name.text == supplied.path[1].text)
                            .expect("checked supplied patch field exists")
                            * 2
                            + 1;
                        let value_index = patch_fields.len() * 2 + index + 1;
                        sqlite_set.push(format!(
                            "{column} = CASE WHEN ?{supplied_index} THEN ?{value_index} ELSE {column} END"
                        ));
                    } else {
                        postgres_set.push(format!("{column} = ${{replacement{index}}}"));
                        let value_index = patch_fields.len() * 2 + index + 1;
                        sqlite_set.push(format!("{column} = ?{value_index}"));
                    }
                }
                let postgres_set = postgres_set.join(", ");
                let sqlite_set = sqlite_set.join(", ");
                let sqlite_predicate = patch_fields.len() * 2 + derived.len() + 1;
                let postgres_update = format!(
                    "UPDATE {table} SET {postgres_set} WHERE {} = ${{predicateValue}} RETURNING {returned_fields}",
                    sql_identifier(&update.field.text)
                );
                let sqlite_update = format!(
                    "UPDATE {table} SET {sqlite_set} WHERE {} = ?{sqlite_predicate} RETURNING {returned_fields}",
                    sql_identifier(&update.field.text)
                );
                let mut patch_arguments = patch_fields
                    .iter()
                    .flat_map(|field| {
                        let key = ts_string(&field.name.text);
                        [
                            format!("hasOwn(patchValue, {key})"),
                            format!("hasOwn(patchValue, {key}) ? patchValue[{key}] : null"),
                        ]
                    })
                    .collect::<Vec<_>>();
                patch_arguments.extend(replacement_names.iter().cloned());
                let patch_arguments = patch_arguments.join(", ");
                let signature_suffix = if replacements.is_empty() {
                    String::new()
                } else {
                    format!(", {}", replacements.join(", "))
                };
                let call_suffix = if replacement_names.is_empty() {
                    String::new()
                } else {
                    format!(", {}", replacement_names.join(", "))
                };
                let operation = format!("update.{name}.{}.patch", update.field.text);
                line(
                    &mut output,
                    &format!(
                        "  async {method}(predicateValue: unknown, patchValue: Record<string, unknown>{signature_suffix}): Promise<unknown | null> {{"
                    ),
                );
                line(&mut output, "    if (postgres !== null) {");
                line(&mut output, "      const execute = async (tx: SQL) => {");
                line(
                    &mut output,
                    &format!("        const matches = await tx`{postgres_select} FOR UPDATE`;"),
                );
                line(&mut output, "        if (matches.length > 1) throw new PersistenceFault(\"update.cardinality\", \"cardinality\", new Error(\"required patch update matched more than one row\"));");
                line(
                    &mut output,
                    "        if (matches.length === 0) return null;",
                );
                line(
                    &mut output,
                    &format!("        const rows = await tx`{postgres_update}`;"),
                );
                line(&mut output, "        if (rows.length !== 1) throw new PersistenceFault(\"update.cardinality\", \"cardinality\", new Error(\"required patch update changed an unexpected row count\"));");
                line(&mut output, "        return rows[0];");
                line(&mut output, "      };");
                line(
                    &mut output,
                    &format!(
                        "      return persistenceAsync({}, () => transactional ? execute(postgres) : postgres.begin(tx => execute(tx as SQL)));",
                        ts_string(&operation)
                    ),
                );
                line(&mut output, "    }");
                line(
                    &mut output,
                    &format!("    const execute = (predicateValue: unknown, patchValue: Record<string, unknown>{signature_suffix}) => {{"),
                );
                line(
                    &mut output,
                    &format!(
                        "      const matches = sqlite!.prepare({}).all(predicateValue);",
                        ts_string(&sqlite_select)
                    ),
                );
                line(&mut output, "      if (matches.length > 1) throw new PersistenceFault(\"update.cardinality\", \"cardinality\", new Error(\"required patch update matched more than one row\"));");
                line(&mut output, "      if (matches.length === 0) return null;");
                line(
                    &mut output,
                    &format!(
                        "      const rows = sqlite!.prepare({}).all({patch_arguments}, predicateValue);",
                        ts_string(&sqlite_update)
                    ),
                );
                line(&mut output, "      if (rows.length !== 1) throw new PersistenceFault(\"update.cardinality\", \"cardinality\", new Error(\"required patch update changed an unexpected row count\"));");
                line(&mut output, "      return rows[0];");
                line(&mut output, "    };");
                line(
                    &mut output,
                    &format!("    if (transactional) return persistenceSync(\"update.transactional\", () => execute(predicateValue, patchValue{call_suffix}));"),
                );
                line(
                    &mut output,
                    "    const mutate = sqlite!.transaction(execute);",
                );
                line(
                    &mut output,
                    &format!(
                        "    return persistenceSync({}, () => mutate(predicateValue, patchValue{call_suffix}));",
                        ts_string(&operation)
                    ),
                );
                line(&mut output, "  },");
            }
        }

        let mut emitted_multi_updates = BTreeSet::new();
        for callable in self.callables.values() {
            let mut updates = Vec::new();
            collect_update_expressions(&callable.body, &mut updates);
            for update in updates
                .into_iter()
                .filter(|update| update.patch.is_none() && update.changes.len() > 1)
            {
                let name = update
                    .target
                    .path
                    .iter()
                    .map(|part| part.text.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                let change_suffix = update
                    .changes
                    .iter()
                    .map(|change| change.name.text.as_str())
                    .collect::<Vec<_>>()
                    .join("_and_");
                let method = format!(
                    "update_required_{name}_by_{}_set_{change_suffix}",
                    update.field.text
                );
                if !emitted_multi_updates.insert(method.clone()) {
                    continue;
                }
                let entity = self
                    .records
                    .get(&name)
                    .expect("checked update entity exists");
                let table = sql_identifier(&snake_case(&name));
                let fields = entity
                    .fields
                    .iter()
                    .map(|field| sql_identifier(&field.name.text))
                    .collect::<Vec<_>>()
                    .join(", ");
                let postgres_select = self
                    .select_optional_sql(&name, entity, &update.field.text, SqlDialect::Postgres)
                    .replace("$1", "${predicateValue}");
                let sqlite_select =
                    self.select_optional_sql(&name, entity, &update.field.text, SqlDialect::Sqlite);
                let replacements = (0..update.changes.len())
                    .map(|index| format!("replacement{index}: unknown"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let replacement_names = (0..update.changes.len())
                    .map(|index| format!("replacement{index}"))
                    .collect::<Vec<_>>();
                let postgres_set = update
                    .changes
                    .iter()
                    .enumerate()
                    .map(|(index, change)| {
                        format!(
                            "{} = ${{replacement{index}}}",
                            sql_identifier(&change.name.text)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let sqlite_set = update
                    .changes
                    .iter()
                    .enumerate()
                    .map(|(index, change)| {
                        format!("{} = ?{}", sql_identifier(&change.name.text), index + 1)
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let postgres_update = format!(
                    "UPDATE {table} SET {postgres_set} WHERE {} = ${{predicateValue}} RETURNING {fields}",
                    sql_identifier(&update.field.text)
                );
                let sqlite_update = format!(
                    "UPDATE {table} SET {sqlite_set} WHERE {} = ?{} RETURNING {fields}",
                    sql_identifier(&update.field.text),
                    update.changes.len() + 1
                );
                let operation = format!("update.{name}.{}.multi", update.field.text);
                line(
                    &mut output,
                    &format!(
                        "  async {method}(predicateValue: unknown, {replacements}): Promise<unknown | null> {{"
                    ),
                );
                line(&mut output, "    if (postgres !== null) {");
                line(&mut output, "      const execute = async (tx: SQL) => {");
                line(
                    &mut output,
                    &format!("        const matches = await tx`{postgres_select} FOR UPDATE`;"),
                );
                line(&mut output, "        if (matches.length > 1) throw new PersistenceFault(\"update.cardinality\", \"cardinality\", new Error(\"required update matched more than one row\"));");
                line(
                    &mut output,
                    "        if (matches.length === 0) return null;",
                );
                line(
                    &mut output,
                    &format!("        const rows = await tx`{postgres_update}`;"),
                );
                line(&mut output, "        if (rows.length !== 1) throw new PersistenceFault(\"update.cardinality\", \"cardinality\", new Error(\"required update changed an unexpected row count\"));");
                line(&mut output, "        return rows[0];");
                line(&mut output, "      };");
                line(
                    &mut output,
                    &format!(
                        "      return persistenceAsync({}, () => transactional ? execute(postgres) : postgres.begin(tx => execute(tx as SQL)));",
                        ts_string(&operation)
                    ),
                );
                line(&mut output, "    }");
                line(
                    &mut output,
                    &format!("    const execute = (predicateValue: unknown, {replacements}) => {{"),
                );
                line(
                    &mut output,
                    &format!(
                        "      const matches = sqlite!.prepare({}).all(predicateValue);",
                        ts_string(&sqlite_select)
                    ),
                );
                line(&mut output, "      if (matches.length > 1) throw new PersistenceFault(\"update.cardinality\", \"cardinality\", new Error(\"required update matched more than one row\"));");
                line(&mut output, "      if (matches.length === 0) return null;");
                line(
                    &mut output,
                    &format!(
                        "      const rows = sqlite!.prepare({}).all({}, predicateValue);",
                        ts_string(&sqlite_update),
                        replacement_names.join(", ")
                    ),
                );
                line(&mut output, "      if (rows.length !== 1) throw new PersistenceFault(\"update.cardinality\", \"cardinality\", new Error(\"required update changed an unexpected row count\"));");
                line(&mut output, "      return rows[0];");
                line(&mut output, "    };");
                let arguments = format!("predicateValue, {}", replacement_names.join(", "));
                line(
                    &mut output,
                    &format!(
                        "    if (transactional) return persistenceSync(\"update.transactional\", () => execute({arguments}));"
                    ),
                );
                line(
                    &mut output,
                    "    const mutate = sqlite!.transaction(execute);",
                );
                line(
                    &mut output,
                    &format!(
                        "    return persistenceSync({}, () => mutate({arguments}));",
                        ts_string(&operation)
                    ),
                );
                line(&mut output, "  },");
            }
        }
        for (child_name, child) in self.entities() {
            for relationship in child
                .fields
                .iter()
                .filter(|field| field.reference.is_some())
            {
                let reference = relationship
                    .reference
                    .as_ref()
                    .expect("filtered owning reference exists");
                let relationship_name = owning_relationship_name(relationship);
                let parent_name = &reference.target.path[0].text;
                let target_field = &reference.target.path[1].text;
                for predicate in &child.fields {
                    for cardinality in ["required", "optional"] {
                        let method = format!(
                            "query_required_{child_name}_with_{}_{cardinality}_by_{}",
                            relationship_name, predicate.name.text
                        );
                        line(
                            &mut output,
                            &format!(
                                "  async {method}(value: unknown): Promise<unknown | null> {{"
                            ),
                        );
                        line(
                            &mut output,
                            &format!(
                                "    const parent = await this.query_optional_{child_name}_by_{}(value);",
                                predicate.name.text
                            ),
                        );
                        line(&mut output, "    if (parent === null) return null;");
                        line(
                            &mut output,
                            &format!(
                                "    const foreignValue = (parent as Record<string, unknown>)[{}];",
                                ts_string(&relationship.name.text)
                            ),
                        );
                        if cardinality == "optional" {
                            line(
                                &mut output,
                                &format!(
                                    "    if (foreignValue === null) return {{ parent, {}: null }};",
                                    relationship_name
                                ),
                            );
                        }
                        line(
                            &mut output,
                            &format!(
                                "    const related = await this.query_optional_{parent_name}_by_{target_field}(foreignValue);"
                            ),
                        );
                        line(
                            &mut output,
                            "    if (related === null) throw new PersistenceFault(\"query.relationship\", \"cardinality\", new Error(\"owning reference target is missing\"));",
                        );
                        line(
                            &mut output,
                            &format!("    return {{ parent, {}: related }};", relationship_name),
                        );
                        line(&mut output, "  },");
                    }
                }
                if !relationship.field_type.nullable {
                    if let Some(parent) = self.records.get(parent_name) {
                        for nested in parent.inverses.iter().filter(|inverse| {
                            inverse.cardinality == jadpo_syntax::InverseCardinality::Optional
                        }) {
                            let Some(nested_via_field) =
                                nested.via.path.get(1).map(|part| part.text.as_str())
                            else {
                                continue;
                            };
                            let Some(nested_target_field) = self
                                .records
                                .get(&nested.target.text)
                                .and_then(|record| {
                                    record
                                        .fields
                                        .iter()
                                        .find(|field| field.name.text == nested_via_field)
                                })
                                .and_then(|field| field.reference.as_ref())
                                .and_then(|reference| reference.target.path.get(1))
                                .map(|part| part.text.as_str())
                            else {
                                continue;
                            };
                            for predicate in &child.fields {
                                let method = format!(
                                    "query_required_{child_name}_with_{}_then_{}_optional_by_{}",
                                    relationship_name, nested.name.text, predicate.name.text
                                );
                                line(
                                    &mut output,
                                    &format!(
                                        "  async {method}(value: unknown): Promise<unknown | null> {{"
                                    ),
                                );
                                line(
                                    &mut output,
                                    &format!(
                                        "    const root = await this.query_optional_{child_name}_by_{}(value);",
                                        predicate.name.text
                                    ),
                                );
                                line(&mut output, "    if (root === null) return null;");
                                line(
                                    &mut output,
                                    &format!(
                                        "    const owner = await this.query_optional_{parent_name}_by_{target_field}((root as Record<string, unknown>)[{}]);",
                                        ts_string(&relationship.name.text)
                                    ),
                                );
                                line(
                                    &mut output,
                                    "    if (owner === null) throw new PersistenceFault(\"query.relationship\", \"cardinality\", new Error(\"nested owning reference target is missing\"));",
                                );
                                line(
                                    &mut output,
                                    &format!(
                                        "    const leaf = await this.query_optional_{}_by_{nested_via_field}((owner as Record<string, unknown>)[{}]);",
                                        nested.target.text,
                                        ts_string(nested_target_field)
                                    ),
                                );
                                line(
                                    &mut output,
                                    &format!(
                                        "    return {{ parent: root, {}: {{ parent: owner, {}: leaf }} }};",
                                        relationship_name, nested.name.text
                                    ),
                                );
                                line(&mut output, "  },");
                            }
                        }
                    }
                }
            }
        }
        for (parent_name, parent) in self.entities() {
            for inverse in &parent.inverses {
                let Some(child) = self.records.get(&inverse.target.text) else {
                    continue;
                };
                let Some(via_field) = inverse.via.path.get(1).map(|part| part.text.as_str()) else {
                    continue;
                };
                let Some(target_field) = child
                    .fields
                    .iter()
                    .find(|field| field.name.text == via_field)
                    .and_then(|field| field.reference.as_ref())
                    .and_then(|reference| reference.target.path.get(1))
                    .map(|name| name.text.as_str())
                else {
                    continue;
                };
                if inverse.cardinality == jadpo_syntax::InverseCardinality::Optional {
                    for predicate in &parent.fields {
                        let method = format!(
                            "query_required_{parent_name}_with_{}_optional_by_{}",
                            inverse.name.text, predicate.name.text
                        );
                        line(
                            &mut output,
                            &format!(
                                "  async {method}(value: unknown): Promise<unknown | null> {{"
                            ),
                        );
                        line(
                            &mut output,
                            &format!(
                                "    const parent = await this.query_optional_{parent_name}_by_{}(value);",
                                predicate.name.text
                            ),
                        );
                        line(&mut output, "    if (parent === null) return null;");
                        line(
                            &mut output,
                            &format!(
                                "    const related = await this.query_optional_{}_by_{via_field}((parent as Record<string, unknown>)[{}]);",
                                inverse.target.text,
                                ts_string(target_field)
                            ),
                        );
                        line(
                            &mut output,
                            &format!("    return {{ parent, {}: related }};", inverse.name.text),
                        );
                        line(&mut output, "  },");
                    }
                    continue;
                }
                for predicate in &parent.fields {
                    for order in child.fields.iter().filter(|field| {
                        has_modifier(field, PersistenceModifier::Identity)
                            || has_modifier(field, PersistenceModifier::Unique)
                    }) {
                        for direction in ["asc", "desc"] {
                            let method = format!(
                                "query_required_{parent_name}_with_{}_by_{}_order_by_{}_{direction}",
                                inverse.name.text, predicate.name.text, order.name.text
                            );
                            let child_method = format!(
                                "query_many_{}_by_{via_field}_order_by_{}_{direction}_paginated",
                                inverse.target.text, order.name.text
                            );
                            line(
                                &mut output,
                                &format!(
                                    "  async {method}(value: unknown, limit: unknown, offset: unknown): Promise<unknown | null> {{"
                                ),
                            );
                            line(
                                &mut output,
                                &format!(
                                    "    const parent = await this.query_optional_{parent_name}_by_{}(value);",
                                    predicate.name.text
                                ),
                            );
                            line(&mut output, "    if (parent === null) return null;");
                            line(
                                &mut output,
                                &format!(
                                    "    const children = await this.{child_method}((parent as Record<string, unknown>)[{}], limit, offset);",
                                    ts_string(target_field)
                                ),
                            );
                            line(
                                &mut output,
                                &format!(
                                    "    return {{ parent, {}: children }};",
                                    inverse.name.text
                                ),
                            );
                            line(&mut output, "  },");
                        }
                    }
                }
                let Some(child_presence) = child.fields.iter().find(|field| {
                    has_modifier(field, PersistenceModifier::Identity)
                        || has_modifier(field, PersistenceModifier::Unique)
                }) else {
                    continue;
                };
                let parent_object = parent
                    .fields
                    .iter()
                    .map(|field| {
                        format!(
                            "{}: row[{}]",
                            field.name.text,
                            ts_string(&format!("parent__{}", field.name.text))
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let child_object = child
                    .fields
                    .iter()
                    .map(|field| {
                        format!(
                            "{}: row[{}]",
                            field.name.text,
                            ts_string(&format!("child__{}", field.name.text))
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                for predicate in &parent.fields {
                    for parent_order in parent.fields.iter().filter(|field| {
                        has_modifier(field, PersistenceModifier::Identity)
                            || has_modifier(field, PersistenceModifier::Unique)
                    }) {
                        for child_order in child.fields.iter().filter(|field| {
                            has_modifier(field, PersistenceModifier::Identity)
                                || has_modifier(field, PersistenceModifier::Unique)
                        }) {
                            for parent_direction in ["asc", "desc"] {
                                for child_direction in ["asc", "desc"] {
                                    let method = format!(
                                        "query_many_{parent_name}_with_{}_by_{}_order_by_{}_{parent_direction}_include_order_by_{}_{child_direction}_paginated",
                                        inverse.name.text,
                                        predicate.name.text,
                                        parent_order.name.text,
                                        child_order.name.text
                                    );
                                    let postgres_sql = self.select_many_with_inverse_sql(
                                        parent_name,
                                        parent,
                                        &predicate.name.text,
                                        &parent_order.name.text,
                                        &parent_direction.to_ascii_uppercase(),
                                        &inverse.target.text,
                                        child,
                                        via_field,
                                        target_field,
                                        &child_order.name.text,
                                        &child_direction.to_ascii_uppercase(),
                                        SqlDialect::Postgres,
                                    );
                                    let sqlite_sql = self.select_many_with_inverse_sql(
                                        parent_name,
                                        parent,
                                        &predicate.name.text,
                                        &parent_order.name.text,
                                        &parent_direction.to_ascii_uppercase(),
                                        &inverse.target.text,
                                        child,
                                        via_field,
                                        target_field,
                                        &child_order.name.text,
                                        &child_direction.to_ascii_uppercase(),
                                        SqlDialect::Sqlite,
                                    );
                                    let postgres_template = postgres_sql
                                        .replace("$1", "${value}")
                                        .replace("$2", "${parentLimit}")
                                        .replace("$3", "${parentOffset}")
                                        .replace("$4", "${childLimit}")
                                        .replace("$5", "${childOffset}");
                                    line(
                                        &mut output,
                                        &format!(
                                            "  async {method}(value: unknown, parentLimit: unknown, parentOffset: unknown, childLimit: unknown, childOffset: unknown): Promise<unknown[]> {{"
                                        ),
                                    );
                                    line(&mut output, "    const rows = postgres !== null");
                                    line(
                                        &mut output,
                                        &format!(
                                            "      ? await persistenceAsync({}, () => postgres`{postgres_template}`)",
                                            ts_string(&format!(
                                                "query_many_include.{parent_name}.{}",
                                                inverse.name.text
                                            ))
                                        ),
                                    );
                                    line(
                                        &mut output,
                                        &format!(
                                            "      : persistenceSync({}, () => sqlite!.prepare({}).all(value, parentLimit, parentOffset, childLimit, childOffset));",
                                            ts_string(&format!(
                                                "query_many_include.{parent_name}.{}",
                                                inverse.name.text
                                            )),
                                            ts_string(&sqlite_sql)
                                        ),
                                    );
                                    line(
                                        &mut output,
                                        &format!(
                                            "    const grouped = new Map<unknown, {{ parent: Record<string, unknown>; {}: Record<string, unknown>[] }}>();",
                                            inverse.name.text
                                        ),
                                    );
                                    line(&mut output, "    for (const raw of rows) {");
                                    line(
                                        &mut output,
                                        "      const row = raw as Record<string, unknown>;",
                                    );
                                    line(
                                        &mut output,
                                        &format!("      const parent = {{ {parent_object} }};"),
                                    );
                                    line(
                                        &mut output,
                                        &format!(
                                            "      const key = parent[{}];",
                                            ts_string(target_field)
                                        ),
                                    );
                                    line(&mut output, "      let entry = grouped.get(key);");
                                    line(&mut output, "      if (entry === undefined) {");
                                    line(
                                        &mut output,
                                        &format!(
                                            "        entry = {{ parent, {}: [] }};",
                                            inverse.name.text
                                        ),
                                    );
                                    line(&mut output, "        grouped.set(key, entry);");
                                    line(&mut output, "      }");
                                    line(
                                        &mut output,
                                        &format!(
                                            "      if (row[{}] !== null && row[{}] !== undefined) entry.{}.push({{ {child_object} }});",
                                            ts_string(&format!(
                                                "child__{}",
                                                child_presence.name.text
                                            )),
                                            ts_string(&format!(
                                                "child__{}",
                                                child_presence.name.text
                                            )),
                                            inverse.name.text
                                        ),
                                    );
                                    line(&mut output, "    }");
                                    line(&mut output, "    return [...grouped.values()];");
                                    line(&mut output, "  },");
                                }
                            }
                        }
                    }
                }
            }
        }
        line(&mut output, "};");
        line(&mut output, "return client;");
        line(&mut output, "}");
        line(&mut output, "");
        line(
            &mut output,
            "export const persistence = createPersistenceClient(postgres, sqlite);",
        );
        output
    }

    fn table_body(
        &self,
        name: &str,
        entity: &RecordDeclaration,
        dialect: SqlDialect,
        indent: &str,
        separator: &str,
    ) -> String {
        let mut parts = entity
            .fields
            .iter()
            .map(|field| {
                let nullable = if field.field_type.nullable {
                    ""
                } else {
                    " NOT NULL"
                };
                format!(
                    "{indent}{} {}{nullable}",
                    sql_identifier(&field.name.text),
                    self.sql_type(&field.field_type, dialect)
                )
            })
            .collect::<Vec<_>>();
        for field in &entity.fields {
            if has_modifier(field, PersistenceModifier::Identity) {
                parts.push(format!(
                    "{indent}CONSTRAINT {} PRIMARY KEY ({})",
                    sql_identifier(&format!("{}_identity", snake_case(name))),
                    sql_identifier(&field.name.text)
                ));
            }
            if has_modifier(field, PersistenceModifier::Unique) {
                parts.push(format!(
                    "{indent}CONSTRAINT {} UNIQUE ({})",
                    sql_identifier(&format!(
                        "{}_{}_unique",
                        snake_case(name),
                        snake_case(&field.name.text)
                    )),
                    sql_identifier(&field.name.text)
                ));
            }
            if let Some(reference) = &field.reference {
                let target_entity = &reference.target.path[0].text;
                let target_field = &reference.target.path[1].text;
                parts.push(format!(
                    "{indent}CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {} ({}) ON DELETE {}",
                    sql_identifier(&format!(
                        "{}_{}_fk",
                        snake_case(name),
                        snake_case(&field.name.text)
                    )),
                    sql_identifier(&field.name.text),
                    sql_identifier(&snake_case(target_entity)),
                    sql_identifier(target_field),
                    reference_delete_sql(reference.on_delete)
                ));
            }
        }
        for constraint in &entity.persistence_constraints {
            let columns = constraint
                .fields
                .iter()
                .map(|field| sql_identifier(&field.text))
                .collect::<Vec<_>>()
                .join(", ");
            parts.push(format!(
                "{indent}CONSTRAINT {} UNIQUE ({columns})",
                sql_identifier(&format!(
                    "{}_{}_unique",
                    snake_case(name),
                    snake_case(&constraint.name.text)
                ))
            ));
        }
        parts.join(separator)
    }

    fn index_statements(&self, name: &str, entity: &RecordDeclaration) -> Vec<String> {
        let table = sql_identifier(&snake_case(name));
        entity
            .fields
            .iter()
            .filter(|field| {
                has_modifier(field, PersistenceModifier::Index) || field.reference.is_some()
            })
            .map(|field| {
                let index = sql_identifier(&format!(
                    "{}_{}_idx",
                    snake_case(name),
                    snake_case(&field.name.text)
                ));
                let column = sql_identifier(&field.name.text);
                format!("CREATE INDEX IF NOT EXISTS {index} ON {table} ({column});")
            })
            .collect()
    }

    fn insert_sql(&self, name: &str, entity: &RecordDeclaration, dialect: SqlDialect) -> String {
        let table = sql_identifier(&snake_case(name));
        let fields = entity
            .fields
            .iter()
            .map(|field| sql_identifier(&field.name.text))
            .collect::<Vec<_>>();
        let placeholders = (1..=fields.len())
            .map(|index| match dialect {
                SqlDialect::Postgres => format!("${index}"),
                SqlDialect::Sqlite => format!("?{index}"),
            })
            .collect::<Vec<_>>();
        format!(
            "INSERT INTO {table} ({}) VALUES ({}) RETURNING {}",
            fields.join(", "),
            placeholders.join(", "),
            fields.join(", ")
        )
    }

    fn select_optional_sql(
        &self,
        name: &str,
        entity: &RecordDeclaration,
        predicate_field: &str,
        dialect: SqlDialect,
    ) -> String {
        let table = sql_identifier(&snake_case(name));
        let fields = entity
            .fields
            .iter()
            .map(|field| sql_identifier(&field.name.text))
            .collect::<Vec<_>>()
            .join(", ");
        let placeholder = match dialect {
            SqlDialect::Postgres => "$1",
            SqlDialect::Sqlite => "?1",
        };
        format!(
            "SELECT {fields} FROM {table} WHERE {} = {placeholder} LIMIT 2",
            sql_identifier(predicate_field)
        )
    }

    fn select_many_sql(
        &self,
        name: &str,
        entity: &RecordDeclaration,
        predicate_field: &str,
        order_field: &str,
        direction: &str,
        dialect: SqlDialect,
    ) -> String {
        let table = sql_identifier(&snake_case(name));
        let fields = entity
            .fields
            .iter()
            .map(|field| sql_identifier(&field.name.text))
            .collect::<Vec<_>>()
            .join(", ");
        let placeholder = match dialect {
            SqlDialect::Postgres => "$1",
            SqlDialect::Sqlite => "?1",
        };
        format!(
            "SELECT {fields} FROM {table} WHERE {} = {placeholder} ORDER BY {} {direction}",
            sql_identifier(predicate_field),
            sql_identifier(order_field)
        )
    }

    fn select_many_paginated_sql(
        &self,
        name: &str,
        entity: &RecordDeclaration,
        predicate_field: &str,
        order_field: &str,
        direction: &str,
        dialect: SqlDialect,
    ) -> String {
        let table = sql_identifier(&snake_case(name));
        let fields = entity
            .fields
            .iter()
            .map(|field| sql_identifier(&field.name.text))
            .collect::<Vec<_>>()
            .join(", ");
        let (predicate, limit, offset) = match dialect {
            SqlDialect::Postgres => ("$1", "$2", "$3"),
            SqlDialect::Sqlite => ("?1", "?2", "?3"),
        };
        format!(
            "SELECT {fields} FROM {table} WHERE {} = {predicate} ORDER BY {} {direction} LIMIT {limit} OFFSET {offset}",
            sql_identifier(predicate_field),
            sql_identifier(order_field)
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn select_many_with_inverse_sql(
        &self,
        parent_name: &str,
        parent: &RecordDeclaration,
        predicate_field: &str,
        parent_order_field: &str,
        parent_direction: &str,
        child_name: &str,
        child: &RecordDeclaration,
        via_field: &str,
        target_field: &str,
        child_order_field: &str,
        child_direction: &str,
        dialect: SqlDialect,
    ) -> String {
        let parent_table = sql_identifier(&snake_case(parent_name));
        let child_table = sql_identifier(&snake_case(child_name));
        let parent_fields = parent
            .fields
            .iter()
            .map(|field| sql_identifier(&field.name.text))
            .collect::<Vec<_>>()
            .join(", ");
        let child_fields = child
            .fields
            .iter()
            .map(|field| sql_identifier(&field.name.text))
            .collect::<Vec<_>>()
            .join(", ");
        let selected_parent = parent
            .fields
            .iter()
            .map(|field| {
                format!(
                    "\"parent_page\".{} AS {}",
                    sql_identifier(&field.name.text),
                    sql_identifier(&format!("parent__{}", field.name.text))
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let selected_child = child
            .fields
            .iter()
            .map(|field| {
                format!(
                    "\"child_page\".{} AS {}",
                    sql_identifier(&field.name.text),
                    sql_identifier(&format!("child__{}", field.name.text))
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let (predicate, parent_limit, parent_offset, child_limit, child_offset) = match dialect {
            SqlDialect::Postgres => ("$1", "$2", "$3", "$4", "$5"),
            SqlDialect::Sqlite => ("?1", "?2", "?3", "?4", "?5"),
        };
        format!(
            "WITH \"parent_page\" AS (SELECT {parent_fields} FROM {parent_table} WHERE {} = {predicate} ORDER BY {} {parent_direction} LIMIT {parent_limit} OFFSET {parent_offset}), \"child_page\" AS (SELECT {child_fields}, ROW_NUMBER() OVER (PARTITION BY {} ORDER BY {} {child_direction}) AS \"__row_number\" FROM {child_table} WHERE {} IN (SELECT {} FROM \"parent_page\")) SELECT {selected_parent}, {selected_child} FROM \"parent_page\" LEFT JOIN \"child_page\" ON \"child_page\".{} = \"parent_page\".{} AND \"child_page\".\"__row_number\" > {child_offset} AND \"child_page\".\"__row_number\" <= ({child_offset} + {child_limit}) ORDER BY \"parent_page\".{} {parent_direction}, \"child_page\".{} {child_direction}",
            sql_identifier(predicate_field),
            sql_identifier(parent_order_field),
            sql_identifier(via_field),
            sql_identifier(child_order_field),
            sql_identifier(via_field),
            sql_identifier(target_field),
            sql_identifier(via_field),
            sql_identifier(target_field),
            sql_identifier(parent_order_field),
            sql_identifier(child_order_field)
        )
    }

    fn sql_type(&self, reference: &TypeReference, dialect: SqlDialect) -> &'static str {
        match self.representation_type(&type_name(reference)).as_str() {
            "Bool" => match dialect {
                SqlDialect::Postgres => "BOOLEAN",
                SqlDialect::Sqlite => "INTEGER",
            },
            "Int" => "BIGINT",
            "Decimal" => match dialect {
                SqlDialect::Postgres => "NUMERIC",
                SqlDialect::Sqlite => "REAL",
            },
            "Text" | "Uuid" | "DateTime" => "TEXT",
            _ => "BLOB",
        }
    }

    fn representation_type(&self, raw_name: &str) -> String {
        let mut current = raw_name.trim_end_matches('?').to_owned();
        if self.enums.contains_key(&current) {
            return "Text".to_owned();
        }
        let mut visited = BTreeSet::new();
        while visited.insert(current.clone()) {
            let Some(node) = self.project.semantics.node(&current) else {
                break;
            };
            let Some(edge) = self
                .project
                .semantics
                .refinements
                .iter()
                .find(|edge| edge.refined == node.id)
            else {
                break;
            };
            current.clone_from(&self.project.semantics.nodes[edge.parent.0 as usize].name);
        }
        current
    }

    fn runtime_prelude(&self, output: &mut String) {
        line(output, "type JsonObject = Record<string, unknown>;");
        line(
            output,
            "declare const safeOperationalTextBrand: unique symbol;",
        );
        line(
            output,
            "type SafeOperationalText = string & { readonly [safeOperationalTextBrand]: true };",
        );
        line(
            output,
            "type SafeOperationalValue = SafeOperationalText | number | boolean | null;",
        );
        line(output, "export type OperationalLogEvent = {");
        line(output, "  schemaVersion: 1;");
        line(output, "  kind: \"operational_log_event\";");
        line(output, "  eventName: string;");
        line(output, "  classification: string;");
        line(output, "  requestId: string;");
        line(output, "  traceId: string | null;");
        line(output, "  semanticOperationId: string;");
        line(output, "  sourceRevision: string;");
        line(
            output,
            "  attributes: Record<string, SafeOperationalValue>;",
        );
        line(output, "};");
        line(output, "");
        line(output, "class ValidationError extends Error {}");
        line(output, "");
        line(output, "class DomainFailure extends Error {");
        line(output, "  constructor(");
        line(output, "    readonly failureName: string,");
        line(output, "    readonly publicContext: JsonObject,");
        line(output, "    readonly internalContext: JsonObject,");
        line(output, "  ) {");
        line(output, "    super(failureName);");
        line(output, "  }");
        line(output, "}");
        line(output, "");
        line(
            output,
            "function matchRoutePath(template: string, actual: string): JsonObject | null {",
        );
        line(output, "  const expected = template.split(\"/\");");
        line(output, "  const received = actual.split(\"/\");");
        line(
            output,
            "  if (expected.length !== received.length) return null;",
        );
        line(output, "  const values: JsonObject = {};");
        line(
            output,
            "  for (let index = 0; index < expected.length; index += 1) {",
        );
        line(output, "    const segment = expected[index];");
        line(output, "    const value = received[index];");
        line(
            output,
            "    if (segment.startsWith(\"{\") && segment.endsWith(\"}\")) {",
        );
        line(output, "      try { values[segment.slice(1, -1)] = decodeURIComponent(value); } catch { return null; }");
        line(output, "    } else if (segment !== value) return null;");
        line(output, "  }");
        line(output, "  return values;");
        line(output, "}");
        line(output, "");
        line(
            output,
            "function invalid(path: string, expectation: string): never {",
        );
        line(
            output,
            "  throw new ValidationError(`${path} must satisfy ${expectation}`);",
        );
        line(output, "}");
        line(output, "");
        line(
            output,
            "function expectObject(value: unknown, path: string): JsonObject {",
        );
        line(
            output,
            "  if (typeof value !== \"object\" || value === null || Array.isArray(value)) invalid(path, \"a closed object\");",
        );
        line(output, "  return value as JsonObject;");
        line(output, "}");
        line(output, "");
        line(
            output,
            "function rejectUnknownFields(value: JsonObject, allowed: readonly string[], path: string): void {",
        );
        line(output, "  const allowedSet = new Set(allowed);");
        line(output, "  for (const key of Object.keys(value)) {");
        line(
            output,
            "    if (!allowedSet.has(key)) invalid(`${path}.${key}`, \"a declared field\");",
        );
        line(output, "  }");
        line(output, "}");
        line(output, "");
        line(
            output,
            "function hasOwn(value: JsonObject, key: string): boolean {",
        );
        line(
            output,
            "  return Object.prototype.hasOwnProperty.call(value, key);",
        );
        line(output, "}");
        line(output, "");
        line(
            output,
            "function validateText(value: unknown, path: string): string {",
        );
        line(
            output,
            "  if (typeof value !== \"string\") invalid(path, \"Text\");",
        );
        line(output, "  return value;");
        line(output, "}");
        line(output, "");
        line(
            output,
            "function validateBool(value: unknown, path: string): boolean {",
        );
        line(
            output,
            "  if (typeof value !== \"boolean\") invalid(path, \"Bool\");",
        );
        line(output, "  return value;");
        line(output, "}");
        line(output, "");
        line(
            output,
            "function validateInt(value: unknown, path: string): number {",
        );
        line(
            output,
            "  if (typeof value !== \"number\" || !Number.isInteger(value)) invalid(path, \"Int\");",
        );
        line(output, "  return value;");
        line(output, "}");
        line(output, "");
        line(
            output,
            "function validateDecimal(value: unknown, path: string): number {",
        );
        line(
            output,
            "  if (typeof value !== \"number\" || !Number.isFinite(value)) invalid(path, \"Decimal\");",
        );
        line(output, "  return value;");
        line(output, "}");
        line(output, "");
        line(
            output,
            "function validateUuid(value: unknown, path: string): string {",
        );
        line(
            output,
            "  if (typeof value !== \"string\" || !/^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/iu.test(value)) invalid(path, \"Uuid\");",
        );
        line(output, "  return value;");
        line(output, "}");
        line(output, "");
        line(
            output,
            "function validateDateTime(value: unknown, path: string): string {",
        );
        line(
            output,
            "  if (typeof value !== \"string\" || Number.isNaN(Date.parse(value))) invalid(path, \"DateTime\");",
        );
        line(output, "  return value;");
        line(output, "}");
        line(output, "");
        line(output, "function isEmail(value: string): boolean {");
        line(output, "  const parts = value.split(\"@\");");
        line(output, "  if (parts.length !== 2) return false;");
        line(output, "  const [local, domain] = parts;");
        line(
            output,
            "  return local.length > 0 && domain.includes(\".\") && !domain.startsWith(\".\") && !domain.endsWith(\".\") && !/\\s/u.test(value);",
        );
        line(output, "}");
        line(output, "");
        line(
            output,
            "function json(status: number, body: unknown, requestId: string): Response {",
        );
        line(output, "  return Response.json(body, {");
        line(output, "    status,");
        line(
            output,
            "    headers: { \"x-request-id\": requestId, \"cache-control\": \"no-store\" },",
        );
        line(output, "  });");
        line(output, "}");
        line(output, "");
        line(
            output,
            "function errorEnvelope(code: string, message: string, requestId: string, details?: JsonObject): JsonObject {",
        );
        line(output, "  return {");
        line(output, "    error: {");
        line(output, "      code,");
        line(output, "      message,");
        line(output, "      request_id: requestId,");
        line(
            output,
            "      ...(details === undefined ? {} : { details }),",
        );
        line(output, "    },");
        line(output, "  };");
        line(output, "}");
        line(output, "");
        line(
            output,
            "export function operationalEventToOpenTelemetry(event: OperationalLogEvent): JsonObject {",
        );
        line(output, "  return { name: event.eventName, attributes: { classification: event.classification, request_id: event.requestId, trace_id: event.traceId, semantic_operation_id: event.semanticOperationId, source_revision: event.sourceRevision, ...event.attributes } };");
        line(output, "}");
        line(output, "");
        line(
            output,
            "export function operationalEventToProvider(event: OperationalLogEvent): JsonObject {",
        );
        line(output, "  return { fingerprint: [event.classification, event.semanticOperationId, event.sourceRevision], tags: { operation: event.semanticOperationId, release: event.sourceRevision }, extra: event.attributes };");
        line(output, "}");
        line(output, "");
        line(
            output,
            "export function reportRuntimeFault(classification: string, semanticOperationId: string, sourceRevision: string, requestId: string, error: unknown): void {",
        );
        line(output, "  const event: OperationalLogEvent = {");
        line(output, "    schemaVersion: 1,");
        line(output, "    kind: \"operational_log_event\",");
        line(output, "    eventName: \"operation.failed\",");
        line(output, "    classification,");
        line(output, "    requestId,");
        line(output, "    traceId: null,");
        line(output, "    semanticOperationId,");
        line(output, "    sourceRevision,");
        line(output, "    attributes: {},");
        line(output, "  };");
        line(output, "  console.error(JSON.stringify(event));");
        line(
            output,
            "  if (Bun.env.JADPO_DEBUG_TARGET_STACKS === \"1\") console.error(error);",
        );
        line(output, "}");
        line(output, "");
    }

    fn type_aliases(&self, output: &mut String) {
        for (name, declaration) in &self.enums {
            let tagged = enum_is_tagged(declaration);
            let variants = declaration
                .variants
                .iter()
                .map(|variant| {
                    if tagged {
                        let mut fields =
                            vec![format!("readonly tag: {}", ts_string(&variant.name.text))];
                        fields.extend(variant.fields.iter().map(|field| {
                            format!(
                                "{}{}: {}",
                                field.name.text,
                                if field.optional { "?" } else { "" },
                                self.ts_type(&field.field_type)
                            )
                        }));
                        format!("{{ {} }}", fields.join("; "))
                    } else {
                        ts_string(&variant.name.text)
                    }
                })
                .collect::<Vec<_>>()
                .join(" | ");
            line(output, &format!("type {name} = {variants};"));
        }
        if !self.enums.is_empty() {
            line(output, "");
        }
        for (name, declaration) in &self.types {
            line(
                output,
                &format!(
                    "type {name} = {} & {{ readonly __brand_{name}: unique symbol }};",
                    self.ts_type(&declaration.parent)
                ),
            );
        }
        if !self.types.is_empty() {
            line(output, "");
        }
        for (name, declaration) in &self.records {
            line(output, &format!("type {name} = {{"));
            for field in &declaration.fields {
                let optional = if field.optional { "?" } else { "" };
                line(
                    output,
                    &format!(
                        "  {}{optional}: {};",
                        field.name.text,
                        self.ts_type(&field.field_type)
                    ),
                );
            }
            line(output, "};");
            line(output, "");
        }
    }

    fn validators(&self, output: &mut String) {
        for (name, declaration) in &self.enums {
            line(
                output,
                &format!("function validate_{name}(value: unknown, path: string): {name} {{"),
            );
            if enum_is_tagged(declaration) {
                line(output, "  const object = expectObject(value, path);");
                line(
                    output,
                    "  const tag = validateText(object[\"tag\"], `${path}.tag`);",
                );
                line(output, "  switch (tag) {");
                for variant in &declaration.variants {
                    line(
                        output,
                        &format!("    case {}: {{", ts_string(&variant.name.text)),
                    );
                    let allowed = std::iter::once(ts_string("tag"))
                        .chain(
                            variant
                                .fields
                                .iter()
                                .map(|field| ts_string(&field.name.text)),
                        )
                        .collect::<Vec<_>>()
                        .join(", ");
                    line(
                        output,
                        &format!("      rejectUnknownFields(object, [{allowed}], path);"),
                    );
                    for field in &variant.fields {
                        if !field.optional {
                            line(
                                output,
                                &format!(
                                    "      if (!hasOwn(object, {})) invalid(`${{path}}.{}`, \"a required field\");",
                                    ts_string(&field.name.text),
                                    field.name.text
                                ),
                            );
                        }
                    }
                    line(output, "      return {");
                    line(
                        output,
                        &format!("        tag: {},", ts_string(&variant.name.text)),
                    );
                    for field in &variant.fields {
                        let value = self.validation_expression(
                            &field.field_type,
                            &format!("object[{}]", ts_string(&field.name.text)),
                            &format!("`${{path}}.{}`", field.name.text),
                        );
                        if field.optional {
                            line(
                                output,
                                &format!(
                                    "        ...(hasOwn(object, {}) ? {{ {}: {value} }} : {{}}),",
                                    ts_string(&field.name.text),
                                    field.name.text
                                ),
                            );
                        } else {
                            line(output, &format!("        {}: {value},", field.name.text));
                        }
                    }
                    line(output, "      };");
                    line(output, "    }");
                }
                line(
                    output,
                    &format!("    default: return invalid(path, {});", ts_string(name)),
                );
                line(output, "  }");
            } else {
                let condition = declaration
                    .variants
                    .iter()
                    .map(|variant| format!("value !== {}", ts_string(&variant.name.text)))
                    .collect::<Vec<_>>()
                    .join(" && ");
                line(
                    output,
                    &format!("  if ({condition}) invalid(path, {});", ts_string(name)),
                );
                line(output, &format!("  return value as {name};"));
            }
            line(output, "}");
            line(output, "");
        }

        for (name, declaration) in &self.types {
            line(
                output,
                &format!("function validate_{name}(value: unknown, path: string): {name} {{"),
            );
            let parent_name = type_name(&declaration.parent);
            if self.types.contains_key(&parent_name) {
                line(
                    output,
                    &format!("  const candidate = validate_{parent_name}(value, path);"),
                );
            } else {
                self.primitive_validation(output, "value", "path", &parent_name, 2);
                line(output, "  const candidate = value;");
            }
            self.constraint_checks(output, "candidate", "path", &declaration.constraints, 2);
            line(output, &format!("  return candidate as {name};"));
            line(output, "}");
            line(output, "");
        }

        for (name, declaration) in &self.records {
            line(
                output,
                &format!("function validate_{name}(value: unknown, path: string): {name} {{"),
            );
            line(output, "  const object = expectObject(value, path);");
            let fields = declaration
                .fields
                .iter()
                .map(|field| ts_string(&field.name.text))
                .collect::<Vec<_>>()
                .join(", ");
            line(
                output,
                &format!("  rejectUnknownFields(object, [{fields}], path);"),
            );
            for field in &declaration.fields {
                if !field.optional {
                    line(
                        output,
                        &format!(
                            "  if (!hasOwn(object, {})) invalid(`${{path}}.{}`, \"a required field\");",
                            ts_string(&field.name.text),
                            field.name.text
                        ),
                    );
                }
            }
            line(output, "  return {");
            for field in &declaration.fields {
                let value = self.validation_expression(
                    &field.field_type,
                    &format!("object[{}]", ts_string(&field.name.text)),
                    &format!("`${{path}}.{}`", field.name.text),
                );
                if field.optional {
                    line(
                        output,
                        &format!(
                            "    ...(hasOwn(object, {}) ? {{ {}: {value} }} : {{}}),",
                            ts_string(&field.name.text),
                            field.name.text
                        ),
                    );
                } else {
                    line(output, &format!("    {}: {value},", field.name.text));
                }
            }
            line(output, "  };");
            line(output, "}");
            line(output, "");
        }
    }

    fn failure_contracts(&self, output: &mut String) {
        line(output, "const failureContracts = {");
        for contract in &self.project.failures.contracts {
            line(output, &format!("  {}: {{", contract.name));
            line(output, &format!("    code: {},", ts_string(&contract.code)));
            line(output, &format!("    status: {},", contract.http_status));
            line(
                output,
                &format!(
                    "    message: {},",
                    contract
                        .message
                        .as_deref()
                        .map(ts_string)
                        .unwrap_or_else(|| "null".to_owned())
                ),
            );
            let fields = contract
                .public_fields
                .iter()
                .map(|field| ts_string(field))
                .collect::<Vec<_>>()
                .join(", ");
            line(output, &format!("    publicFields: [{fields}],"));
            line(output, "  },");
        }
        line(output, "} as const;");
        line(output, "");
    }

    fn callable_implementations(&self, output: &mut String) {
        for declaration in self.callables.values() {
            let mut parameters = declaration
                .parameters
                .iter()
                .map(|parameter| {
                    format!(
                        "{}: {}",
                        parameter.name.text,
                        self.ts_type(&parameter.parameter_type)
                    )
                })
                .collect::<Vec<_>>();
            if self.has_persistence_operations() {
                parameters
                    .push("__persistence: typeof rootPersistence = rootPersistence".to_owned());
            }
            let parameters = parameters.join(", ");
            line(
                output,
                &format!(
                    "async function {}({parameters}): Promise<{}> {{",
                    declaration.name.text,
                    self.ts_type(&declaration.return_type)
                ),
            );
            if self.callable_is_mutative(&declaration.name.text) {
                line(
                    output,
                    "  return __persistence.transaction(async persistence => {",
                );
                self.block(output, &declaration.body, 4);
                line(output, "  });");
            } else {
                if self.has_persistence_operations() {
                    line(output, "  const persistence = __persistence;");
                }
                self.block(output, &declaration.body, 2);
            }
            line(output, "}");
            line(output, "");
        }
    }

    fn test_implementations(&self, output: &mut String) {
        for (index, test) in self.tests.iter().enumerate() {
            line(
                output,
                &format!("async function jadpoTest{index}(): Promise<void> {{"),
            );
            if self.has_persistence_operations() {
                line(output, "  const persistence = rootPersistence;");
            }
            self.block(output, &test.body, 2);
            line(output, "}");
            line(output, "");
        }
        if self.tests.is_empty() {
            return;
        }
        line(output, "export async function runTests() {");
        line(output, "  const tests = [");
        for (index, test) in self.tests.iter().enumerate() {
            line(
                output,
                &format!(
                    "    {{ name: {}, run: jadpoTest{index} }},",
                    ts_string(unquote(&test.name.text))
                ),
            );
        }
        line(output, "  ];");
        line(output, "  const results: Array<{ name: string; status: \"passed\" | \"failed\"; diagnostic?: { code: string; message: string } }> = [];");
        line(output, "  for (const test of tests) {");
        line(output, "    try {");
        line(output, "      await test.run();");
        line(
            output,
            "      results.push({ name: test.name, status: \"passed\" });",
        );
        line(output, "    } catch (error) {");
        line(
            output,
            "      const message = error instanceof Error ? error.message : \"test failed\";",
        );
        line(output, "      results.push({ name: test.name, status: \"failed\", diagnostic: { code: \"TEST_FAILED\", message } });");
        line(output, "    }");
        line(output, "  }");
        line(
            output,
            "  const failed = results.filter(result => result.status === \"failed\").length;",
        );
        line(output, "  return { schema_version: 1, kind: \"test_report\", status: failed === 0 ? \"passed\" : \"failed\", summary: { total: results.length, passed: results.length - failed, failed }, results }; ");
        line(output, "}");
        line(output, "");
    }

    fn test_entrypoint(&self) -> String {
        let mut output = String::new();
        line(&mut output, "// Generated by Jadpo 0.0.1. Do not edit.");
        line(&mut output, "import { runTests } from \"./app.ts\";");
        line(&mut output, "");
        line(&mut output, "const report = await runTests();");
        line(&mut output, "console.log(JSON.stringify(report));");
        line(
            &mut output,
            "if (report.status === \"failed\") process.exit(1);",
        );
        output
    }

    fn has_authored_health_route(&self) -> bool {
        self.project.syntax.sources.iter().any(|source| {
            source.file.declarations.iter().any(|declaration| {
                matches!(
                    declaration,
                    Declaration::Route(route)
                        if route.method == HttpMethod::Get && route.path == "/health"
                )
            })
        })
    }

    fn http_handler(&self, output: &mut String) {
        line(
            output,
            "export async function handleRequest(request: Request): Promise<Response> {",
        );
        line(output, "  const requestId = `req_${crypto.randomUUID()}`;");
        line(
            output,
            &format!(
                "  const sourceRevision = {};",
                ts_string(&self.source_revision())
            ),
        );
        line(output, "  let semanticOperationId = \"http:unmatched\";");
        line(output, "  let requestPath = \"<unparsed>\";");
        line(output, "  try {");
        line(output, "    const url = new URL(request.url);");
        line(output, "    requestPath = url.pathname;");

        if !self.has_authored_health_route() {
            line(
                output,
                "    if (request.method === \"GET\" && url.pathname === \"/health\") {",
            );
            line(
                output,
                "      return json(200, { ready: true }, requestId);",
            );
            line(output, "    }");
        }

        for source in &self.project.syntax.sources {
            for declaration in &source.file.declarations {
                let Declaration::Route(route) = declaration else {
                    continue;
                };
                line(
                    output,
                    &format!(
                        "    const routePath{} = matchRoutePath({}, url.pathname);",
                        route.range.start,
                        ts_string(&route.path)
                    ),
                );
                line(
                    output,
                    &format!(
                        "    if (request.method === {} && routePath{} !== null) {{",
                        ts_string(method_name(route.method)),
                        route.range.start
                    ),
                );
                line(
                    output,
                    &format!(
                        "      semanticOperationId = {};",
                        ts_string(&format!(
                            "route:{}:{}",
                            method_name(route.method),
                            route.path
                        ))
                    ),
                );
                if !route.path_fields.is_empty() {
                    let path_type = route
                        .path_fields
                        .iter()
                        .map(|field| {
                            format!("{}: {}", field.name.text, self.ts_type(&field.field_type))
                        })
                        .collect::<Vec<_>>()
                        .join("; ");
                    line(output, &format!("      let path: {{ {path_type} }};"));
                    line(output, "      try {");
                    line(output, "        path = {");
                    for field in &route.path_fields {
                        let raw = format!(
                            "routePath{}[{}]",
                            route.range.start,
                            ts_string(&field.name.text)
                        );
                        line(
                            output,
                            &format!(
                                "          {}: {},",
                                field.name.text,
                                self.validation_expression(
                                    &field.field_type,
                                    &raw,
                                    &ts_string(&format!("path.{}", field.name.text))
                                )
                            ),
                        );
                    }
                    line(output, "        };");
                    line(output, "      } catch {");
                    line(
                        output,
                        "        return json(400, errorEnvelope(\"invalid_request\", \"Request validation failed.\", requestId), requestId);",
                    );
                    line(output, "      }");
                }
                if let Some(input) = &route.input {
                    line(
                        output,
                        &format!("      let input: {};", self.ts_type(input)),
                    );
                    line(output, "      try {");
                    line(
                        output,
                        "        const body: unknown = await request.json();",
                    );
                    line(
                        output,
                        &format!(
                            "        input = {};",
                            self.validation_expression(input, "body", "\"request.body\"")
                        ),
                    );
                    line(output, "      } catch {");
                    line(
                        output,
                        "        return json(400, errorEnvelope(\"invalid_request\", \"Request validation failed.\", requestId), requestId);",
                    );
                    line(output, "      }");
                }
                if let Some(run) = &route.run {
                    let invocation = self.invocation(run);
                    line(output, &format!("      const output = {invocation};"));
                } else if let Some(action) = &route.inline_action {
                    let mut visiting = BTreeSet::new();
                    if block_contains_mutation(&action.body, &self.callables, &mut visiting) {
                        line(output, "      const output = await rootPersistence.transaction(async persistence => {");
                        self.block(output, &action.body, 8);
                        line(output, "      });");
                    } else {
                        line(output, "      const output = await (async () => {");
                        if self.has_persistence_operations() {
                            line(output, "        const persistence = rootPersistence;");
                        }
                        self.block(output, &action.body, 8);
                        line(output, "      })();");
                    }
                } else {
                    line(output, "      const output = undefined;");
                }
                let validated = route.output.as_ref().map_or_else(
                    || "null".to_owned(),
                    |output_type| {
                        self.validation_expression(output_type, "output", "\"response.body\"")
                    },
                );
                line(
                    output,
                    &format!("      return json(200, {validated}, requestId);"),
                );
                line(output, "    }");
            }
        }

        line(
            output,
            "    return json(404, errorEnvelope(\"route_not_found\", \"Route not found.\", requestId), requestId);",
        );
        line(output, "  } catch (error) {");
        line(output, "    if (error instanceof DomainFailure) {");
        line(
            output,
            "      const contract = failureContracts[error.failureName as keyof typeof failureContracts];",
        );
        line(output, "      if (contract !== undefined) {");
        line(output, "        const details: JsonObject = {};");
        line(
            output,
            "        for (const field of contract.publicFields) {",
        );
        line(
            output,
            "          if (hasOwn(error.publicContext, field)) details[field] = error.publicContext[field];",
        );
        line(output, "        }");
        line(
            output,
            "        const publicDetails = contract.publicFields.length === 0 ? undefined : details;",
        );
        line(
            output,
            "        return json(contract.status, errorEnvelope(contract.code, contract.message ?? \"Request failed.\", requestId, publicDetails), requestId);",
        );
        line(output, "      }");
        line(output, "    }");
        line(output, "    reportRuntimeFault(");
        line(output, "      \"RUNTIME_UNHANDLED_FAULT\",");
        line(output, "      semanticOperationId,");
        line(output, "      sourceRevision,");
        line(output, "      requestId,");
        line(output, "      error,");
        line(output, "    );");
        line(
            output,
            "    return json(500, errorEnvelope(\"internal_fault\", \"An internal error occurred.\", requestId), requestId);",
        );
        line(output, "  }");
        line(output, "}");
        line(output, "");
        line(output, "if (import.meta.main) {");
        line(output, "  const port = Number(Bun.env.PORT ?? \"3000\");");
        line(output, "  try {");
        line(output, "    Bun.serve({ port, fetch: handleRequest });");
        line(output, "    console.log(JSON.stringify({");
        line(output, "      schemaVersion: 1,");
        line(output, "      kind: \"operational_log_event\",");
        line(output, "      eventName: \"runtime.ready\",");
        line(output, "      classification: \"ready\",");
        line(output, "      requestId: \"startup\",");
        line(output, "      traceId: null,");
        line(output, "      semanticOperationId: \"runtime:start\",");
        line(
            output,
            &format!(
                "      sourceRevision: {},",
                ts_string(&self.source_revision())
            ),
        );
        line(output, "      attributes: { port },");
        line(output, "    }));");
        line(output, "  } catch (error) {");
        line(output, "    reportRuntimeFault(");
        line(output, "      \"RUNTIME_STARTUP_FAILED\",");
        line(output, "      \"runtime:start\",");
        line(
            output,
            &format!("      {},", ts_string(&self.source_revision())),
        );
        line(output, "      \"startup\",");
        line(output, "      error,");
        line(output, "    );");
        line(output, "    process.exit(1);");
        line(output, "  }");
        line(output, "}");
    }

    fn block(&self, output: &mut String, block: &Block, indent: usize) {
        for statement in &block.statements {
            let padding = " ".repeat(indent);
            match statement {
                Statement::Binding(binding) => {
                    let keyword = if binding.mutable { "let" } else { "const" };
                    line(
                        output,
                        &format!(
                            "{padding}{keyword} {} = {};",
                            binding.name.text,
                            self.expression(&binding.value)
                        ),
                    );
                }
                Statement::Assignment(assignment) => line(
                    output,
                    &format!(
                        "{padding}{} = {};",
                        assignment.target.text,
                        self.expression(&assignment.value)
                    ),
                ),
                Statement::Return(statement) => line(
                    output,
                    &format!("{padding}return {};", self.expression(&statement.value)),
                ),
                Statement::Reject(statement) => {
                    line(
                        output,
                        &format!(
                            "{padding}throw new DomainFailure({});",
                            self.domain_failure_arguments(statement),
                        ),
                    );
                }
                Statement::If(statement) => {
                    line(
                        output,
                        &format!("{padding}if ({}) {{", self.expression(&statement.condition)),
                    );
                    self.block(output, &statement.then_block, indent + 2);
                    if let Some(else_block) = &statement.else_block {
                        line(output, &format!("{padding}}} else {{"));
                        self.block(output, else_block, indent + 2);
                    }
                    line(output, &format!("{padding}}}"));
                }
                Statement::Match(statement) => {
                    let matched_enum = statement.arms.iter().find_map(|arm| match &arm.pattern {
                        MatchPattern::Name(pattern) => pattern.path.first(),
                        MatchPattern::Variant(pattern) => pattern.target.path.first(),
                        MatchPattern::Literal(_)
                        | MatchPattern::OptionalSome(_)
                        | MatchPattern::Wildcard(_) => None,
                    });
                    let tagged = matched_enum
                        .and_then(|name| self.enums.get(&name.text))
                        .is_some_and(|declaration| enum_is_tagged(declaration));
                    let subject = self.expression(&statement.subject);
                    let needs_binding = tagged
                        || statement
                            .arms
                            .iter()
                            .any(|arm| matches!(arm.pattern, MatchPattern::OptionalSome(_)));
                    let subject_value = if needs_binding {
                        let binding = format!("matchSubject{}", statement.range.start);
                        line(output, &format!("{padding}const {binding} = {subject};"));
                        binding
                    } else {
                        subject.clone()
                    };
                    let switch_value = if tagged {
                        format!("{subject_value} === null ? null : {subject_value}.tag")
                    } else {
                        subject_value.clone()
                    };
                    line(output, &format!("{padding}switch ({switch_value}) {{"));
                    for arm in &statement.arms {
                        match &arm.pattern {
                            MatchPattern::Name(pattern) => {
                                let variant = pattern
                                    .path
                                    .last()
                                    .map(|part| part.text.as_str())
                                    .unwrap_or_default();
                                line(
                                    output,
                                    &format!("{padding}  case {}: {{", ts_string(variant)),
                                );
                            }
                            MatchPattern::Variant(pattern) => {
                                let variant = pattern
                                    .target
                                    .path
                                    .last()
                                    .map(|part| part.text.as_str())
                                    .unwrap_or_default();
                                line(
                                    output,
                                    &format!("{padding}  case {}: {{", ts_string(variant)),
                                );
                                if !pattern.bindings.is_empty() {
                                    let bindings = pattern
                                        .bindings
                                        .iter()
                                        .map(|binding| binding.text.as_str())
                                        .collect::<Vec<_>>()
                                        .join(", ");
                                    line(
                                        output,
                                        &format!(
                                            "{padding}    const {{ {bindings} }} = {subject_value};"
                                        ),
                                    );
                                }
                            }
                            MatchPattern::OptionalSome(pattern) => {
                                line(output, &format!("{padding}  default: {{"));
                                line(
                                    output,
                                    &format!(
                                        "{padding}    const {} = {subject_value};",
                                        pattern.binding.text
                                    ),
                                );
                            }
                            MatchPattern::Literal(pattern) => {
                                let value = if pattern.kind == LiteralKind::None {
                                    "null".to_owned()
                                } else {
                                    pattern.text.clone()
                                };
                                line(output, &format!("{padding}  case {value}: {{"));
                            }
                            MatchPattern::Wildcard(_) => {
                                line(output, &format!("{padding}  default: {{"));
                            }
                        }
                        self.block(output, &arm.body, indent + 4);
                        line(output, &format!("{padding}    break;"));
                        line(output, &format!("{padding}  }}"));
                    }
                    line(output, &format!("{padding}}}"));
                }
                Statement::Assert(statement) => {
                    line(
                        output,
                        &format!(
                            "{padding}if (!({})) throw new Error({});",
                            self.expression(&statement.condition),
                            ts_string(&format!(
                                "assertion failed at source bytes {}..{}",
                                statement.condition.range().start,
                                statement.condition.range().end
                            ))
                        ),
                    );
                }
                Statement::Unsupported(statement) => line(
                    output,
                    &format!(
                        "{padding}throw new Error({});",
                        ts_string(&format!(
                            "unsupported source statement: {}",
                            statement.keyword
                        ))
                    ),
                ),
            }
        }
    }

    fn expression(&self, expression: &Expression) -> String {
        match expression {
            Expression::Literal(literal) => match literal.kind {
                LiteralKind::None => "null".to_owned(),
                _ => literal.text.clone(),
            },
            Expression::Name(name) => name
                .path
                .first()
                .and_then(|first| self.enums.get(&first.text))
                .filter(|_| name.path.len() == 2)
                .and_then(|declaration| name.path.last().map(|variant| (declaration, variant)))
                .map_or_else(
                    || {
                        name.path
                            .iter()
                            .map(|part| part.text.as_str())
                            .collect::<Vec<_>>()
                            .join(".")
                    },
                    |(declaration, variant)| {
                        if enum_is_tagged(declaration) {
                            format!("{{ tag: {} }}", ts_string(&variant.text))
                        } else {
                            ts_string(&variant.text)
                        }
                    },
                ),
            Expression::Invocation(invocation) => self.invocation(invocation),
            Expression::Construction(construction) => {
                let enum_declaration = construction
                    .target
                    .path
                    .first()
                    .and_then(|name| self.enums.get(&name.text))
                    .filter(|_| construction.target.path.len() == 2);
                if let (Some(declaration), Some(variant)) =
                    (enum_declaration, construction.target.path.last())
                {
                    if enum_is_tagged(declaration) {
                        let fields = construction
                            .fields
                            .iter()
                            .map(|field| {
                                format!("{}: {}", field.name.text, self.expression(&field.value))
                            })
                            .collect::<Vec<_>>();
                        let mut entries = vec![format!("tag: {}", ts_string(&variant.text))];
                        entries.extend(fields);
                        format!("{{ {} }}", entries.join(", "))
                    } else {
                        ts_string(&variant.text)
                    }
                } else {
                    self.object_literal(&construction.fields)
                }
            }
            Expression::Create(create) => {
                let name = create
                    .target
                    .path
                    .iter()
                    .map(|part| part.text.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                let operation = format!(
                    "validate_{name}(await persistence.create_{name}({}), {})",
                    self.object_literal(&create.fields),
                    ts_string(&format!("database.{name}"))
                );
                if create.conflicts.is_empty() {
                    operation
                } else {
                    self.constraint_binding_expression(&name, &operation, &create.conflicts)
                }
            }
            Expression::Query(query) => {
                let name = query
                    .target
                    .path
                    .iter()
                    .map(|part| part.text.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                if query.includes.len() > 1 {
                    self.multi_include_expression(query, &name)
                } else if let Some(include) = query.includes.first() {
                    let result = type_name(&include.result);
                    if let Some(nested) = &include.nested_relationship {
                        let cardinality = match include.cardinality {
                            jadpo_syntax::QueryIncludeCardinality::Optional => "optional",
                            jadpo_syntax::QueryIncludeCardinality::Required => "required",
                            jadpo_syntax::QueryIncludeCardinality::Many => "many",
                        };
                        let operation = format!(
                            "await persistence.query_required_{name}_with_{}_then_{}_{cardinality}_by_{}({})",
                            include.relationship.text,
                            nested.text,
                            query.field.text,
                            self.expression(&query.value)
                        );
                        let missing = query
                            .missing
                            .as_ref()
                            .expect("nested includes require a required root query");
                        return format!(
                            "((value: unknown) => {{ if (value === null) throw new DomainFailure({}); return validate_{result}(value, {}); }})({operation})",
                            self.domain_failure_arguments(missing),
                            ts_string(&format!("database.{result}"))
                        );
                    }
                    if include.cardinality != jadpo_syntax::QueryIncludeCardinality::Many {
                        let cardinality = match include.cardinality {
                            jadpo_syntax::QueryIncludeCardinality::Required => "required",
                            jadpo_syntax::QueryIncludeCardinality::Optional => "optional",
                            jadpo_syntax::QueryIncludeCardinality::Many => unreachable!(),
                        };
                        let operation = format!(
                            "await persistence.query_required_{name}_with_{}_{cardinality}_by_{}({})",
                            include.relationship.text,
                            query.field.text,
                            self.expression(&query.value)
                        );
                        let missing = query
                            .missing
                            .as_ref()
                            .expect("owning-parent includes require a required child query");
                        return format!(
                            "((value: unknown) => {{ if (value === null) throw new DomainFailure({}); return validate_{result}(value, {}); }})({operation})",
                            self.domain_failure_arguments(missing),
                            ts_string(&format!("database.{result}"))
                        );
                    }
                    let child_direction = match include.order.direction {
                        jadpo_syntax::QueryOrderDirection::Ascending => "asc",
                        jadpo_syntax::QueryOrderDirection::Descending => "desc",
                    };
                    if query.cardinality == jadpo_syntax::QueryCardinality::Many {
                        let parent_order = query
                            .order
                            .as_ref()
                            .expect("many-parent includes have explicit parent ordering");
                        let parent_pagination = query
                            .pagination
                            .as_ref()
                            .expect("many-parent includes have explicit parent pagination");
                        let parent_direction = match parent_order.direction {
                            jadpo_syntax::QueryOrderDirection::Ascending => "asc",
                            jadpo_syntax::QueryOrderDirection::Descending => "desc",
                        };
                        let operation = format!(
                            "await persistence.query_many_{name}_with_{}_by_{}_order_by_{}_{parent_direction}_include_order_by_{}_{child_direction}_paginated({}, {}, {}, {}, {})",
                            include.relationship.text,
                            query.field.text,
                            parent_order.field.text,
                            include.order.field.text,
                            self.expression(&query.value),
                            self.expression(&parent_pagination.limit),
                            self.expression(&parent_pagination.offset),
                            self.expression(&include.pagination.limit),
                            self.expression(&include.pagination.offset)
                        );
                        format!(
                            "({operation}).map((value: unknown, index: number) => validate_{result}(value, `database.{result}[${{index}}]`))"
                        )
                    } else {
                        let operation = format!(
                            "await persistence.query_required_{name}_with_{}_by_{}_order_by_{}_{child_direction}({}, {}, {})",
                            include.relationship.text,
                            query.field.text,
                            include.order.field.text,
                            self.expression(&query.value),
                            self.expression(&include.pagination.limit),
                            self.expression(&include.pagination.offset)
                        );
                        let missing = query.missing.as_ref().expect(
                            "included required parent queries have a missing failure binding",
                        );
                        format!(
                            "((value: unknown) => {{ if (value === null) throw new DomainFailure({}); return validate_{result}(value, {}); }})({operation})",
                            self.domain_failure_arguments(missing),
                            ts_string(&format!("database.{result}"))
                        )
                    }
                } else {
                    match query.cardinality {
                    jadpo_syntax::QueryCardinality::Optional => format!(
                        "((value: unknown) => value === null ? null : validate_{name}(value, {}))(await persistence.query_optional_{name}_by_{}({}))",
                        ts_string(&format!("database.{name}")),
                        query.field.text,
                        self.expression(&query.value)
                    ),
                    jadpo_syntax::QueryCardinality::Required => {
                        let missing = query
                            .missing
                            .as_ref()
                            .expect("required queries have a missing failure binding");
                        format!(
                            "((value: unknown) => {{ if (value === null) throw new DomainFailure({}); return validate_{name}(value, {}); }})(await persistence.query_required_{name}_by_{}({}))",
                            self.domain_failure_arguments(missing),
                            ts_string(&format!("database.{name}")),
                            query.field.text,
                            self.expression(&query.value)
                        )
                    }
                    jadpo_syntax::QueryCardinality::Many => {
                        let order = query
                            .order
                            .as_ref()
                            .expect("many-result queries have explicit ordering");
                        let direction = match order.direction {
                            jadpo_syntax::QueryOrderDirection::Ascending => "asc",
                            jadpo_syntax::QueryOrderDirection::Descending => "desc",
                        };
                        let (suffix, arguments) = query.pagination.as_ref().map_or_else(
                            || ("", self.expression(&query.value)),
                            |pagination| {
                                (
                                    "_paginated",
                                    format!(
                                        "{}, {}, {}",
                                        self.expression(&query.value),
                                        self.expression(&pagination.limit),
                                        self.expression(&pagination.offset)
                                    ),
                                )
                            },
                        );
                        format!(
                            "(await persistence.query_many_{name}_by_{}_order_by_{}_{direction}{suffix}({arguments})).map((value: unknown, index: number) => validate_{name}(value, `database.{name}[${{index}}]`))",
                            query.field.text,
                            order.field.text,
                        )
                    }
                }
                }
            }
            Expression::Update(update) => {
                let name = update
                    .target
                    .path
                    .iter()
                    .map(|part| part.text.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                if let Some(patch) = &update.patch {
                    let patch_fields = self
                        .patch_fields_for_update(update)
                        .expect("checked patch input resolves to a record");
                    let suffix = patch_method_suffix(update, &patch_fields);
                    let patch_value = patch
                        .path
                        .iter()
                        .map(|part| part.text.as_str())
                        .collect::<Vec<_>>()
                        .join(".");
                    let supplied = patch_fields
                        .iter()
                        .map(|field| format!("hasOwn(patchValue, {})", ts_string(&field.name.text)))
                        .collect::<Vec<_>>()
                        .join(" || ");
                    let derived_arguments = patch_derived_changes(update)
                        .iter()
                        .map(|(change, _)| self.expression(&change.value))
                        .collect::<Vec<_>>();
                    let derived_suffix = if derived_arguments.is_empty() {
                        String::new()
                    } else {
                        format!(", {}", derived_arguments.join(", "))
                    };
                    let operation = format!(
                        "persistence.update_required_{name}_by_{}_{suffix}({}, patchValue{derived_suffix})",
                        update.field.text,
                        self.expression(&update.value),
                    );
                    let mutation = self.required_mutation_expression(
                        &name,
                        &operation,
                        &update.missing,
                        &update.conflicts,
                    );
                    let empty = update
                        .empty
                        .as_ref()
                        .expect("checked patch updates bind an empty failure");
                    return format!(
                        "(await (async () => {{ const patchValue = {patch_value}; if (!({supplied})) throw new DomainFailure({}); return {mutation}; }})())",
                        self.domain_failure_arguments(empty),
                    );
                }
                let changes = update
                    .changes
                    .iter()
                    .map(|change| change.name.text.as_str())
                    .collect::<Vec<_>>()
                    .join("_and_");
                let replacements = update
                    .changes
                    .iter()
                    .map(|change| self.expression(&change.value))
                    .collect::<Vec<_>>()
                    .join(", ");
                let operation = format!(
                    "persistence.update_required_{name}_by_{}_set_{changes}({}, {replacements})",
                    update.field.text,
                    self.expression(&update.value),
                );
                self.required_mutation_expression(
                    &name,
                    &operation,
                    &update.missing,
                    &update.conflicts,
                )
            }
            Expression::Delete(delete) => {
                let name = delete
                    .target
                    .path
                    .iter()
                    .map(|part| part.text.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                let operation = format!(
                    "persistence.delete_required_{name}_by_{}({})",
                    delete.field.text,
                    self.expression(&delete.value)
                );
                self.required_mutation_expression(
                    &name,
                    &operation,
                    &delete.missing,
                    &delete.conflicts,
                )
            }
            Expression::Unary(unary) => format!(
                "({}{})",
                match unary.operator {
                    jadpo_syntax::UnaryOperator::Not => "!",
                    jadpo_syntax::UnaryOperator::Negate => "-",
                },
                self.expression(&unary.value)
            ),
            Expression::Binary(binary) => format!(
                "({} {} {})",
                self.expression(&binary.left),
                match binary.operator {
                    BinaryOperator::Or => "||",
                    BinaryOperator::And => "&&",
                    BinaryOperator::Equal => "===",
                    BinaryOperator::NotEqual => "!==",
                    BinaryOperator::Less => "<",
                    BinaryOperator::LessEqual => "<=",
                    BinaryOperator::Greater => ">",
                    BinaryOperator::GreaterEqual => ">=",
                    BinaryOperator::Add => "+",
                    BinaryOperator::Subtract => "-",
                    BinaryOperator::Multiply => "*",
                    BinaryOperator::Divide => "/",
                    BinaryOperator::Remainder => "%",
                },
                self.expression(&binary.right)
            ),
            Expression::Grouped(grouped) => format!("({})", self.expression(&grouped.value)),
            Expression::Attempt(attempt) => self.expression(&attempt.value),
            Expression::Missing(_) => "undefined".to_owned(),
        }
    }

    fn patch_fields_for_update(
        &self,
        update: &jadpo_syntax::UpdateExpression,
    ) -> Option<Vec<&'project FieldDeclaration>> {
        let patch = update.patch.as_ref()?;
        let binding = patch.path.first()?.text.as_str();
        for callable in self.callables.values() {
            let mut updates = Vec::new();
            collect_update_expressions(&callable.body, &mut updates);
            if !updates
                .iter()
                .any(|candidate| std::ptr::eq(*candidate, update))
            {
                continue;
            }
            let parameter = callable
                .parameters
                .iter()
                .find(|parameter| parameter.name.text == binding)?;
            let record = self.records.get(&type_name(&parameter.parameter_type))?;
            return Some(record.fields.iter().collect());
        }
        None
    }

    fn multi_include_expression(
        &self,
        query: &jadpo_syntax::QueryExpression,
        parent_name: &str,
    ) -> String {
        let first = query
            .includes
            .first()
            .expect("multi-include queries contain at least two includes");
        let result = type_name(&first.result);
        let parent = self
            .records
            .get(parent_name)
            .expect("checked include parent exists");

        if query.cardinality == jadpo_syntax::QueryCardinality::Required {
            let missing = query
                .missing
                .as_ref()
                .expect("required multi-include query has a missing binding");
            let mut loads = Vec::new();
            let mut fields = Vec::new();
            for include in &query.includes {
                let inverse = parent
                    .inverses
                    .iter()
                    .find(|inverse| inverse.name.text == include.relationship.text)
                    .expect("checked inverse exists");
                let child = self
                    .records
                    .get(&inverse.target.text)
                    .expect("checked inverse child exists");
                let via_field = inverse
                    .via
                    .path
                    .get(1)
                    .expect("checked inverse has a child field")
                    .text
                    .as_str();
                let target_field = child
                    .fields
                    .iter()
                    .find(|field| field.name.text == via_field)
                    .and_then(|field| field.reference.as_ref())
                    .and_then(|reference| reference.target.path.get(1))
                    .expect("checked inverse points at a parent field")
                    .text
                    .as_str();
                let direction = match include.order.direction {
                    jadpo_syntax::QueryOrderDirection::Ascending => "asc",
                    jadpo_syntax::QueryOrderDirection::Descending => "desc",
                };
                loads.push(format!(
                    "const {} = await persistence.query_many_{}_by_{via_field}_order_by_{}_{direction}_paginated((parent as Record<string, unknown>)[{}], {}, {});",
                    include.relationship.text,
                    inverse.target.text,
                    include.order.field.text,
                    ts_string(target_field),
                    self.expression(&include.pagination.limit),
                    self.expression(&include.pagination.offset)
                ));
                fields.push(format!(
                    "{}: {}",
                    include.relationship.text, include.relationship.text
                ));
            }
            return format!(
                "(await (async () => {{ const parent = await persistence.query_optional_{parent_name}_by_{}({}); if (parent === null) throw new DomainFailure({}); {} return validate_{result}({{ parent, {} }}, {}); }})())",
                query.field.text,
                self.expression(&query.value),
                self.domain_failure_arguments(missing),
                loads.join(" "),
                fields.join(", "),
                ts_string(&format!("database.{result}"))
            );
        }

        let parent_order = query
            .order
            .as_ref()
            .expect("many-parent multi-include query has parent ordering");
        let parent_pagination = query
            .pagination
            .as_ref()
            .expect("many-parent multi-include query has parent pagination");
        let parent_direction = match parent_order.direction {
            jadpo_syntax::QueryOrderDirection::Ascending => "asc",
            jadpo_syntax::QueryOrderDirection::Descending => "desc",
        };
        let empty_fields = query
            .includes
            .iter()
            .map(|include| format!("{}: []", include.relationship.text))
            .collect::<Vec<_>>()
            .join(", ");
        let mut batches = Vec::new();
        let mut merges = Vec::new();
        for (index, include) in query.includes.iter().enumerate() {
            let direction = match include.order.direction {
                jadpo_syntax::QueryOrderDirection::Ascending => "asc",
                jadpo_syntax::QueryOrderDirection::Descending => "desc",
            };
            batches.push(format!(
                "const batch_{index} = await persistence.query_many_{parent_name}_with_{}_by_{}_order_by_{}_{parent_direction}_include_order_by_{}_{direction}_paginated({}, {}, {}, {}, {});",
                include.relationship.text,
                query.field.text,
                parent_order.field.text,
                include.order.field.text,
                self.expression(&query.value),
                self.expression(&parent_pagination.limit),
                self.expression(&parent_pagination.offset),
                self.expression(&include.pagination.limit),
                self.expression(&include.pagination.offset)
            ));
            merges.push(format!(
                "for (const raw of batch_{index}) {{ const item = raw as {{ parent: Record<string, unknown>; {}: unknown[] }}; const key = item.parent[{}]; let entry = merged.get(key); if (entry === undefined) {{ entry = {{ parent: item.parent, {empty_fields} }}; merged.set(key, entry); }} entry[{}] = item.{}; }}",
                include.relationship.text,
                ts_string(&parent_order.field.text),
                ts_string(&include.relationship.text),
                include.relationship.text
            ));
        }
        format!(
            "(await (async () => {{ {} const merged = new Map<unknown, Record<string, unknown>>(); {} return [...merged.values()].map((value: unknown, index: number) => validate_{result}(value, `database.{result}[${{index}}]`)); }})())",
            batches.join(" "),
            merges.join(" ")
        )
    }

    fn required_mutation_expression(
        &self,
        entity: &str,
        operation: &str,
        missing: &jadpo_syntax::RejectStatement,
        conflicts: &[jadpo_syntax::ConflictBinding],
    ) -> String {
        let conflict_checks = self.constraint_binding_checks(entity, conflicts);
        format!(
            "(await (async () => {{ try {{ const value = await {operation}; if (value === null) throw new DomainFailure({}); return validate_{entity}(value, {}); }} catch (error) {{ {conflict_checks} throw error; }} }})())",
            self.domain_failure_arguments(missing),
            ts_string(&format!("database.{entity}")),
        )
    }

    fn constraint_binding_expression(
        &self,
        entity: &str,
        operation: &str,
        conflicts: &[jadpo_syntax::ConflictBinding],
    ) -> String {
        let conflict_checks = self.constraint_binding_checks(entity, conflicts);
        format!(
            "(await (async () => {{ try {{ return {operation}; }} catch (error) {{ {conflict_checks} throw error; }} }})())",
        )
    }

    fn constraint_binding_checks(
        &self,
        entity: &str,
        conflicts: &[jadpo_syntax::ConflictBinding],
    ) -> String {
        let mut checks = conflicts
            .iter()
            .filter_map(|binding| {
                binding.constraint.as_ref().map(|constraint| {
                    format!(
                        "if (error instanceof PersistenceFault && error.kind === \"constraint\" && error.constraint === {}) throw new DomainFailure({});",
                        ts_string(&if constraint.path.len() == 1 {
                            format!("{entity}.{}", constraint.path[0].text)
                        } else {
                            constraint
                                .path
                                .iter()
                                .map(|part| part.text.as_str())
                                .collect::<Vec<_>>()
                                .join(".")
                        }),
                        self.domain_failure_arguments(&binding.rejection),
                    )
                })
            })
            .collect::<Vec<_>>();
        if let Some(binding) = conflicts
            .iter()
            .find(|binding| binding.constraint.is_none())
        {
            checks.push(format!(
                "if (error instanceof PersistenceFault && error.kind === \"constraint\") throw new DomainFailure({});",
                self.domain_failure_arguments(&binding.rejection),
            ));
        }
        checks.join(" ")
    }

    fn invocation(&self, invocation: &jadpo_syntax::InvocationExpression) -> String {
        let name = invocation
            .callee
            .path
            .iter()
            .map(|part| part.text.as_str())
            .collect::<Vec<_>>()
            .join(".");
        let arguments = invocation
            .arguments
            .iter()
            .map(|argument| self.expression(argument))
            .collect::<Vec<_>>()
            .join(", ");
        if self.types.contains_key(&name) {
            let argument = invocation
                .arguments
                .first()
                .map(|argument| self.expression(argument))
                .unwrap_or_else(|| "undefined".to_owned());
            format!("validate_{name}({argument}, {})", ts_string(&name))
        } else if self.callables.contains_key(&name) {
            if self.has_persistence_operations() {
                let separator = if arguments.is_empty() { "" } else { ", " };
                format!("await {name}({arguments}{separator}persistence)")
            } else {
                format!("await {name}({arguments})")
            }
        } else {
            format!("{name}({arguments})")
        }
    }

    fn object_literal(&self, fields: &[FieldInitialiser]) -> String {
        self.object_literal_from(fields.iter())
    }

    fn object_literal_from<'field>(
        &self,
        fields: impl IntoIterator<Item = &'field FieldInitialiser>,
    ) -> String {
        let fields = fields
            .into_iter()
            .map(|field| format!("{}: {}", field.name.text, self.expression(&field.value)))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{{ {fields} }}")
    }

    fn domain_failure_arguments(&self, rejection: &jadpo_syntax::RejectStatement) -> String {
        let Some(declaration) = self.failures.get(&rejection.failure.text) else {
            return format!("{}, {{}}, {{}}", ts_string(&rejection.failure.text),);
        };
        let public_names = declaration
            .public_fields
            .iter()
            .map(|field| field.name.text.as_str())
            .collect::<BTreeSet<_>>();
        let internal_names = declaration
            .internal_fields
            .iter()
            .map(|field| field.name.text.as_str())
            .collect::<BTreeSet<_>>();
        format!(
            "{}, {}, {}",
            ts_string(&rejection.failure.text),
            self.object_literal_from(
                rejection
                    .values
                    .iter()
                    .filter(|field| public_names.contains(field.name.text.as_str())),
            ),
            self.object_literal_from(
                rejection
                    .values
                    .iter()
                    .filter(|field| internal_names.contains(field.name.text.as_str())),
            ),
        )
    }

    fn ts_type(&self, reference: &TypeReference) -> String {
        let name = self.schema_declaration(&type_name(reference));
        let base = if name == "List" && reference.arguments.len() == 1 {
            format!("Array<{}>", self.ts_type(&reference.arguments[0]))
        } else {
            match name.as_str() {
                "Bool" => "boolean".to_owned(),
                "Int" | "Decimal" => "number".to_owned(),
                "Text" | "Uuid" | "DateTime" => "string".to_owned(),
                "Unit" => "void".to_owned(),
                _ => name,
            }
        };
        if self.reference_is_nullable(reference) {
            format!("{base} | null")
        } else {
            base
        }
    }

    fn validation_expression(&self, reference: &TypeReference, value: &str, path: &str) -> String {
        let name = self.schema_declaration(&type_name(reference));
        let validation = if name == "List" && reference.arguments.len() == 1 {
            let item = self.validation_expression(
                &reference.arguments[0],
                "item",
                &format!("{path} + \"[\" + index + \"]\""),
            );
            format!(
                "Array.isArray({value}) ? {value}.map((item, index) => {item}) : invalid({path}, \"List\")"
            )
        } else if self.types.contains_key(&name)
            || self.enums.contains_key(&name)
            || self.records.contains_key(&name)
        {
            format!("validate_{name}({value}, {path})")
        } else {
            match name.as_str() {
                "Bool" => format!("validateBool({value}, {path})"),
                "Int" => format!("validateInt({value}, {path})"),
                "Decimal" => format!("validateDecimal({value}, {path})"),
                "Text" => format!("validateText({value}, {path})"),
                "Uuid" => format!("validateUuid({value}, {path})"),
                "DateTime" => format!("validateDateTime({value}, {path})"),
                _ => format!("invalid({path}, {})", ts_string(&name)),
            }
        };
        if self.reference_is_nullable(reference) {
            format!("{value} === null ? null : {validation}")
        } else {
            validation
        }
    }

    fn reference_is_nullable(&self, reference: &TypeReference) -> bool {
        reference.nullable
            || (reference.path.len() == 2
                && self
                    .records
                    .get(&reference.path[0].text)
                    .and_then(|record| {
                        record
                            .fields
                            .iter()
                            .find(|field| field.name.text == reference.path[1].text)
                    })
                    .is_some_and(|field| field.field_type.nullable))
    }

    fn primitive_validation(
        &self,
        output: &mut String,
        value: &str,
        path: &str,
        name: &str,
        indent: usize,
    ) {
        let padding = " ".repeat(indent);
        let check = match name {
            "Text" => format!("typeof {value} !== \"string\""),
            "Bool" => format!("typeof {value} !== \"boolean\""),
            "Int" => format!("typeof {value} !== \"number\" || !Number.isInteger({value})"),
            "Decimal" => format!("typeof {value} !== \"number\" || !Number.isFinite({value})"),
            "Uuid" => format!(
                "typeof {value} !== \"string\" || !/^[0-9a-f]{{8}}-[0-9a-f]{{4}}-[1-5][0-9a-f]{{3}}-[89ab][0-9a-f]{{3}}-[0-9a-f]{{12}}$/iu.test({value})"
            ),
            "DateTime" => format!(
                "typeof {value} !== \"string\" || Number.isNaN(Date.parse({value}))"
            ),
            other => {
                line(
                    output,
                    &format!("{padding}invalid({path}, {});", ts_string(other)),
                );
                return;
            }
        };
        line(
            output,
            &format!(
                "{padding}if ({check}) invalid({path}, {});",
                ts_string(name)
            ),
        );
    }

    fn constraint_checks(
        &self,
        output: &mut String,
        value: &str,
        path: &str,
        constraints: &[Constraint],
        indent: usize,
    ) {
        let padding = " ".repeat(indent);
        for constraint in constraints {
            let check = match constraint.kind {
                ConstraintKind::Min => format!("{value} < {}", constraint.value.text),
                ConstraintKind::Max => format!("{value} > {}", constraint.value.text),
                ConstraintKind::MinLength => {
                    format!("{value}.length < {}", constraint.value.text)
                }
                ConstraintKind::MaxLength => {
                    format!("{value}.length > {}", constraint.value.text)
                }
                ConstraintKind::Pattern if unquote(&constraint.value.text) == "[a-z0-9_]+" => {
                    format!("!/^[a-z0-9_]+$/u.test({value})")
                }
                ConstraintKind::Pattern => continue,
                ConstraintKind::Format if unquote(&constraint.value.text) == "email" => {
                    format!("!isEmail({value})")
                }
                ConstraintKind::Format => continue,
            };
            line(
                output,
                &format!(
                    "{padding}if ({check}) invalid({path}, {});",
                    ts_string(constraint_name(constraint.kind))
                ),
            );
        }
    }

    fn schema_declaration(&self, raw_name: &str) -> String {
        let name = raw_name.trim_end_matches('?');
        if self.types.contains_key(name)
            || self.enums.contains_key(name)
            || self.records.contains_key(name)
        {
            return name.to_owned();
        }
        let mut current = name.to_owned();
        let mut visited = BTreeSet::new();
        while visited.insert(current.clone()) {
            let Some(node) = self.project.semantics.node(&current) else {
                break;
            };
            let Some(edge) = self
                .project
                .semantics
                .refinements
                .iter()
                .find(|edge| edge.refined == node.id)
            else {
                break;
            };
            current.clone_from(&self.project.semantics.nodes[edge.parent.0 as usize].name);
            if self.types.contains_key(&current)
                || self.enums.contains_key(&current)
                || self.records.contains_key(&current)
            {
                return current;
            }
        }
        current
    }
}

fn type_name(reference: &TypeReference) -> String {
    reference
        .path
        .iter()
        .map(|part| part.text.as_str())
        .collect::<Vec<_>>()
        .join(".")
}

fn method_name(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Delete => "DELETE",
    }
}

fn constraint_name(kind: ConstraintKind) -> &'static str {
    match kind {
        ConstraintKind::Min => "minimum",
        ConstraintKind::Max => "maximum",
        ConstraintKind::MinLength => "minimum length",
        ConstraintKind::MaxLength => "maximum length",
        ConstraintKind::Pattern => "declared pattern",
        ConstraintKind::Format => "declared format",
    }
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
}

fn ts_string(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    format!("\"{escaped}\"")
}

fn line(output: &mut String, value: &str) {
    output.push_str(value);
    output.push('\n');
}

fn collect_query_expressions<'expression>(
    block: &'expression Block,
    queries: &mut Vec<&'expression jadpo_syntax::QueryExpression>,
) {
    for statement in &block.statements {
        match statement {
            Statement::Binding(statement) => {
                collect_queries_from_expression(&statement.value, queries)
            }
            Statement::Assignment(statement) => {
                collect_queries_from_expression(&statement.value, queries)
            }
            Statement::Return(statement) => {
                collect_queries_from_expression(&statement.value, queries)
            }
            Statement::Reject(statement) => {
                for field in &statement.values {
                    collect_queries_from_expression(&field.value, queries);
                }
            }
            Statement::If(statement) => {
                collect_queries_from_expression(&statement.condition, queries);
                collect_query_expressions(&statement.then_block, queries);
                if let Some(block) = &statement.else_block {
                    collect_query_expressions(block, queries);
                }
            }
            Statement::Match(statement) => {
                collect_queries_from_expression(&statement.subject, queries);
                for arm in &statement.arms {
                    collect_query_expressions(&arm.body, queries);
                }
            }
            Statement::Assert(statement) => {
                collect_queries_from_expression(&statement.condition, queries)
            }
            Statement::Unsupported(_) => {}
        }
    }
}

fn collect_queries_from_expression<'expression>(
    expression: &'expression Expression,
    queries: &mut Vec<&'expression jadpo_syntax::QueryExpression>,
) {
    match expression {
        Expression::Query(query) => {
            queries.push(query);
            collect_queries_from_expression(&query.value, queries);
            if let Some(pagination) = &query.pagination {
                collect_queries_from_expression(&pagination.limit, queries);
                collect_queries_from_expression(&pagination.offset, queries);
            }
            for include in &query.includes {
                collect_queries_from_expression(&include.pagination.limit, queries);
                collect_queries_from_expression(&include.pagination.offset, queries);
            }
        }
        Expression::Invocation(invocation) => {
            for argument in &invocation.arguments {
                collect_queries_from_expression(argument, queries);
            }
        }
        Expression::Construction(construction) => {
            for field in &construction.fields {
                collect_queries_from_expression(&field.value, queries);
            }
        }
        Expression::Create(create) => {
            for field in &create.fields {
                collect_queries_from_expression(&field.value, queries);
            }
        }
        Expression::Update(update) => {
            collect_queries_from_expression(&update.value, queries);
            for field in &update.changes {
                collect_queries_from_expression(&field.value, queries);
            }
            for conditional in &update.conditional_changes {
                collect_queries_from_expression(&conditional.change.value, queries);
            }
            if let Some(empty) = &update.empty {
                for field in &empty.values {
                    collect_queries_from_expression(&field.value, queries);
                }
            }
        }
        Expression::Delete(delete) => collect_queries_from_expression(&delete.value, queries),
        Expression::Binary(binary) => {
            collect_queries_from_expression(&binary.left, queries);
            collect_queries_from_expression(&binary.right, queries);
        }
        Expression::Unary(unary) => collect_queries_from_expression(&unary.value, queries),
        Expression::Grouped(grouped) => collect_queries_from_expression(&grouped.value, queries),
        Expression::Attempt(attempt) => collect_queries_from_expression(&attempt.value, queries),
        Expression::Literal(_) | Expression::Name(_) | Expression::Missing(_) => {}
    }
}

fn collect_update_expressions<'expression>(
    block: &'expression Block,
    updates: &mut Vec<&'expression jadpo_syntax::UpdateExpression>,
) {
    for statement in &block.statements {
        match statement {
            Statement::Binding(statement) => {
                collect_updates_from_expression(&statement.value, updates)
            }
            Statement::Assignment(statement) => {
                collect_updates_from_expression(&statement.value, updates)
            }
            Statement::Return(statement) => {
                collect_updates_from_expression(&statement.value, updates)
            }
            Statement::Reject(statement) => {
                for field in &statement.values {
                    collect_updates_from_expression(&field.value, updates);
                }
            }
            Statement::If(statement) => {
                collect_updates_from_expression(&statement.condition, updates);
                collect_update_expressions(&statement.then_block, updates);
                if let Some(block) = &statement.else_block {
                    collect_update_expressions(block, updates);
                }
            }
            Statement::Match(statement) => {
                collect_updates_from_expression(&statement.subject, updates);
                for arm in &statement.arms {
                    collect_update_expressions(&arm.body, updates);
                }
            }
            Statement::Assert(statement) => {
                collect_updates_from_expression(&statement.condition, updates)
            }
            Statement::Unsupported(_) => {}
        }
    }
}

fn patch_derived_changes(
    update: &jadpo_syntax::UpdateExpression,
) -> Vec<(&FieldInitialiser, Option<&jadpo_syntax::NameExpression>)> {
    update
        .changes
        .iter()
        .map(|change| (change, None))
        .chain(
            update
                .conditional_changes
                .iter()
                .map(|conditional| (&conditional.change, Some(&conditional.supplied))),
        )
        .collect()
}

fn patch_method_suffix(
    update: &jadpo_syntax::UpdateExpression,
    patch_fields: &[&FieldDeclaration],
) -> String {
    let patch = patch_fields
        .iter()
        .map(|field| field.name.text.as_str())
        .collect::<Vec<_>>()
        .join("_and_");
    let derived = patch_derived_changes(update)
        .iter()
        .map(|(change, _)| change.name.text.as_str())
        .collect::<Vec<_>>()
        .join("_and_");
    if derived.is_empty() {
        format!("patch_{patch}")
    } else {
        format!("patch_{patch}_set_{derived}")
    }
}

fn collect_updates_from_expression<'expression>(
    expression: &'expression Expression,
    updates: &mut Vec<&'expression jadpo_syntax::UpdateExpression>,
) {
    match expression {
        Expression::Update(update) => {
            updates.push(update);
            collect_updates_from_expression(&update.value, updates);
            for change in &update.changes {
                collect_updates_from_expression(&change.value, updates);
            }
            for conditional in &update.conditional_changes {
                collect_updates_from_expression(&conditional.change.value, updates);
            }
            if let Some(empty) = &update.empty {
                for field in &empty.values {
                    collect_updates_from_expression(&field.value, updates);
                }
            }
        }
        Expression::Invocation(invocation) => {
            for argument in &invocation.arguments {
                collect_updates_from_expression(argument, updates);
            }
        }
        Expression::Construction(construction) => {
            for field in &construction.fields {
                collect_updates_from_expression(&field.value, updates);
            }
        }
        Expression::Create(create) => {
            for field in &create.fields {
                collect_updates_from_expression(&field.value, updates);
            }
        }
        Expression::Query(query) => {
            collect_updates_from_expression(&query.value, updates);
            if let Some(pagination) = &query.pagination {
                collect_updates_from_expression(&pagination.limit, updates);
                collect_updates_from_expression(&pagination.offset, updates);
            }
            for include in &query.includes {
                collect_updates_from_expression(&include.pagination.limit, updates);
                collect_updates_from_expression(&include.pagination.offset, updates);
            }
        }
        Expression::Delete(delete) => collect_updates_from_expression(&delete.value, updates),
        Expression::Binary(binary) => {
            collect_updates_from_expression(&binary.left, updates);
            collect_updates_from_expression(&binary.right, updates);
        }
        Expression::Unary(unary) => collect_updates_from_expression(&unary.value, updates),
        Expression::Grouped(grouped) => collect_updates_from_expression(&grouped.value, updates),
        Expression::Attempt(attempt) => collect_updates_from_expression(&attempt.value, updates),
        Expression::Literal(_) | Expression::Name(_) | Expression::Missing(_) => {}
    }
}

fn block_contains_persistence(block: &Block) -> bool {
    block.statements.iter().any(|statement| match statement {
        Statement::Binding(statement) => expression_contains_persistence(&statement.value),
        Statement::Assignment(statement) => expression_contains_persistence(&statement.value),
        Statement::Return(statement) => expression_contains_persistence(&statement.value),
        Statement::Reject(statement) => statement
            .values
            .iter()
            .any(|field| expression_contains_persistence(&field.value)),
        Statement::If(statement) => {
            expression_contains_persistence(&statement.condition)
                || block_contains_persistence(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_contains_persistence)
        }
        Statement::Match(statement) => {
            expression_contains_persistence(&statement.subject)
                || statement
                    .arms
                    .iter()
                    .any(|arm| block_contains_persistence(&arm.body))
        }
        Statement::Assert(statement) => expression_contains_persistence(&statement.condition),
        Statement::Unsupported(_) => false,
    })
}

fn expression_contains_persistence(expression: &Expression) -> bool {
    match expression {
        Expression::Create(_)
        | Expression::Query(_)
        | Expression::Update(_)
        | Expression::Delete(_) => true,
        Expression::Invocation(invocation) => invocation
            .arguments
            .iter()
            .any(expression_contains_persistence),
        Expression::Construction(construction) => construction
            .fields
            .iter()
            .any(|field| expression_contains_persistence(&field.value)),
        Expression::Binary(binary) => {
            expression_contains_persistence(&binary.left)
                || expression_contains_persistence(&binary.right)
        }
        Expression::Unary(unary) => expression_contains_persistence(&unary.value),
        Expression::Grouped(grouped) => expression_contains_persistence(&grouped.value),
        Expression::Attempt(attempt) => expression_contains_persistence(&attempt.value),
        Expression::Literal(_) | Expression::Name(_) | Expression::Missing(_) => false,
    }
}

fn callable_is_mutative(
    name: &str,
    callables: &BTreeMap<String, &CallableDeclaration>,
    visiting: &mut BTreeSet<String>,
) -> bool {
    if !visiting.insert(name.to_owned()) {
        return false;
    }
    let result = callables
        .get(name)
        .is_some_and(|callable| block_contains_mutation(&callable.body, callables, visiting));
    visiting.remove(name);
    result
}

fn block_contains_mutation(
    block: &Block,
    callables: &BTreeMap<String, &CallableDeclaration>,
    visiting: &mut BTreeSet<String>,
) -> bool {
    block.statements.iter().any(|statement| match statement {
        Statement::Binding(statement) => {
            expression_contains_mutation(&statement.value, callables, visiting)
        }
        Statement::Assignment(statement) => {
            expression_contains_mutation(&statement.value, callables, visiting)
        }
        Statement::Return(statement) => {
            expression_contains_mutation(&statement.value, callables, visiting)
        }
        Statement::Reject(statement) => statement
            .values
            .iter()
            .any(|field| expression_contains_mutation(&field.value, callables, visiting)),
        Statement::If(statement) => {
            expression_contains_mutation(&statement.condition, callables, visiting)
                || block_contains_mutation(&statement.then_block, callables, visiting)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(|block| block_contains_mutation(block, callables, visiting))
        }
        Statement::Match(statement) => {
            expression_contains_mutation(&statement.subject, callables, visiting)
                || statement
                    .arms
                    .iter()
                    .any(|arm| block_contains_mutation(&arm.body, callables, visiting))
        }
        Statement::Assert(statement) => {
            expression_contains_mutation(&statement.condition, callables, visiting)
        }
        Statement::Unsupported(_) => false,
    })
}

fn expression_contains_mutation(
    expression: &Expression,
    callables: &BTreeMap<String, &CallableDeclaration>,
    visiting: &mut BTreeSet<String>,
) -> bool {
    match expression {
        Expression::Create(_) | Expression::Update(_) | Expression::Delete(_) => true,
        Expression::Query(query) => {
            expression_contains_mutation(&query.value, callables, visiting)
                || query.pagination.as_ref().is_some_and(|pagination| {
                    expression_contains_mutation(&pagination.limit, callables, visiting)
                        || expression_contains_mutation(&pagination.offset, callables, visiting)
                })
                || query.includes.iter().any(|include| {
                    expression_contains_mutation(&include.pagination.limit, callables, visiting)
                        || expression_contains_mutation(
                            &include.pagination.offset,
                            callables,
                            visiting,
                        )
                })
        }
        Expression::Invocation(invocation) => {
            invocation
                .arguments
                .iter()
                .any(|argument| expression_contains_mutation(argument, callables, visiting))
                || invocation
                    .callee
                    .path
                    .first()
                    .is_some_and(|callee| callable_is_mutative(&callee.text, callables, visiting))
        }
        Expression::Construction(construction) => construction
            .fields
            .iter()
            .any(|field| expression_contains_mutation(&field.value, callables, visiting)),
        Expression::Binary(binary) => {
            expression_contains_mutation(&binary.left, callables, visiting)
                || expression_contains_mutation(&binary.right, callables, visiting)
        }
        Expression::Unary(unary) => expression_contains_mutation(&unary.value, callables, visiting),
        Expression::Grouped(grouped) => {
            expression_contains_mutation(&grouped.value, callables, visiting)
        }
        Expression::Attempt(attempt) => {
            expression_contains_mutation(&attempt.value, callables, visiting)
        }
        Expression::Literal(_) | Expression::Name(_) | Expression::Missing(_) => false,
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

fn enum_is_tagged(declaration: &EnumDeclaration) -> bool {
    declaration
        .variants
        .iter()
        .any(|variant| !variant.fields.is_empty())
}

fn sql_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn has_modifier(field: &FieldDeclaration, modifier: PersistenceModifier) -> bool {
    field.persistence.contains(&modifier)
}

fn owning_relationship_name(field: &FieldDeclaration) -> &str {
    field
        .reference
        .as_ref()
        .and_then(|reference| reference.relationship.as_ref())
        .map_or(field.name.text.as_str(), |name| name.text.as_str())
}

const fn reference_delete_name(action: ReferenceDeleteAction) -> &'static str {
    match action {
        ReferenceDeleteAction::Restrict => "restrict",
        ReferenceDeleteAction::Cascade => "cascade",
        ReferenceDeleteAction::SetNull => "set_null",
    }
}

const fn reference_delete_sql(action: ReferenceDeleteAction) -> &'static str {
    match action {
        ReferenceDeleteAction::Restrict => "RESTRICT",
        ReferenceDeleteAction::Cascade => "CASCADE",
        ReferenceDeleteAction::SetNull => "SET NULL",
    }
}

#[cfg(test)]
mod tests {
    use super::{derive_target, validate_runtime_dependency_contract};
    use crate::{analyze_project, create_project, GeneratedArtifact};
    use std::fs;
    use std::path::Path;

    fn repository_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("compiler crate should be inside the repository")
    }

    #[test]
    fn generates_the_seed_runtime_contract() {
        let seed = repository_root().join("examples/jadpo-seed");
        let analyzed = analyze_project(&seed).expect("seed should analyze");
        let targets = derive_target(&seed, &analyzed).expect("seed target should generate");

        assert_eq!(targets.len(), 5);
        assert_eq!(targets[0].relative_path, "target/app.ts");
        assert!(targets[0].contents.contains("function validate_Email"));
        assert!(targets[0]
            .contents
            .contains("request.method === \"POST\" && routePath"));
        assert!(targets[0]
            .contents
            .contains("matchRoutePath(\"/registrations\", url.pathname)"));
        assert!(targets[0].contents.contains("internalContext"));
        assert!(targets[0].contents.contains(
            "throw new DomainFailure(\"InviteCodeRejected\", {  }, { invite_code: input.invite_code });"
        ));
        assert!(!targets[0]
            .contents
            .contains("invite_code: error.internalContext"));
        assert!(targets[0].contents.contains("RUNTIME_UNHANDLED_FAULT"));
        assert!(targets[0].contents.contains("RUNTIME_STARTUP_FAILED"));
        assert!(targets[0].contents.contains("JADPO_DEBUG_TARGET_STACKS"));
        assert!(!targets[0]
            .contents
            .contains("unhandled generated-runtime fault"));
        assert_eq!(
            targets,
            derive_target(&seed, &analyzed).expect("second target should generate")
        );
    }

    #[test]
    fn generates_the_public_health_scaffold() {
        let root =
            std::env::temp_dir().join(format!("jadpo-target-scaffold-{}", std::process::id()));
        let scaffold = root.join("example");
        create_project(&scaffold).expect("scaffold should be created");
        let analyzed = analyze_project(&scaffold).expect("scaffold should analyze");
        let targets = derive_target(&scaffold, &analyzed).expect("target should generate");

        assert!(targets[0]
            .contents
            .contains("request.method === \"GET\" && routePath"));
        assert!(targets[0]
            .contents
            .contains("matchRoutePath(\"/health\", url.pathname)"));
        assert_eq!(
            targets[0]
                .contents
                .matches("matchRoutePath(\"/health\", url.pathname)")
                .count(),
            1,
            "an authored health route should replace the compiler default"
        );
        fs::remove_dir_all(root).expect("temporary scaffold should be removable");
    }

    #[test]
    fn protected_route_generation_names_and_locates_the_route() {
        let root = std::env::temp_dir().join(format!(
            "jadpo-target-protected-route-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("fixture directory should be created");
        let source_path = root.join("app.jadpo");
        fs::write(
            &source_path,
            "output Health { ready: Bool }\nroute GET /health { output: Health action: { return Health { ready: true } } }\n",
        )
        .expect("fixture should be written");
        let analyzed = analyze_project(&root).expect("protected route should analyze");

        let diagnostic =
            derive_target(&root, &analyzed).expect_err("protected route must block generation");

        assert_eq!(diagnostic.code, "JADPO_TARGET_AUTH_NOT_IMPLEMENTED");
        assert_eq!(
            diagnostic.message,
            "Protected route `GET /health` cannot be generated yet"
        );
        assert_eq!(
            diagnostic.context,
            vec![("route".to_owned(), "GET /health".to_owned())]
        );
        let primary = diagnostic
            .primary
            .expect("route should have a source location");
        assert_eq!(primary.source, source_path.to_string_lossy());
        assert_eq!(
            &fs::read_to_string(&source_path).expect("fixture should remain")
                [primary.start..primary.end],
            "route GET /health"
        );
        fs::remove_dir_all(root).expect("fixture should be removable");
    }

    #[test]
    fn generates_mutable_local_reassignment_without_reference_syntax() {
        let fixture =
            repository_root().join("tests/compile/pass/51_mutable_local_reassignment.jadpo");
        let analyzed = analyze_project(&fixture).expect("fixture should analyze");
        assert!(analyzed.syntax.diagnostics().next().is_none());
        assert!(analyzed.semantics.diagnostics.is_empty());
        assert!(analyzed.typing.diagnostics.is_empty());
        assert!(analyzed.failures.diagnostics.is_empty());
        let targets = derive_target(&fixture, &analyzed).expect("target should generate");

        assert!(targets[0]
            .contents
            .contains("let selected = initial;\n  selected = replacement;\n  return selected;"));
    }

    #[test]
    fn generates_enum_validation_and_exhaustive_match_dispatch() {
        let fixture =
            repository_root().join("tests/compile/pass/53_plain_enum_exhaustive_match.jadpo");
        let analyzed = analyze_project(&fixture).expect("enum fixture should analyze");
        assert!(analyzed.syntax.diagnostics().next().is_none());
        assert!(analyzed.semantics.diagnostics.is_empty());
        assert!(analyzed.typing.diagnostics.is_empty());
        let targets = derive_target(&fixture, &analyzed).expect("enum target should generate");
        let application = &targets[0].contents;

        assert!(application.contains("type DeliveryState = \"pending\" | \"sent\" | \"failed\";"));
        assert!(application.contains("function validate_DeliveryState"));
        assert!(application.contains("return \"sent\";"));
        assert!(application.contains("switch (input.state)"));
        assert!(application.contains("case \"pending\":"));
        assert!(application.contains("case \"sent\":"));
        assert!(application.contains("case \"failed\":"));
    }

    #[test]
    fn generates_tagged_sum_validation_construction_and_narrowing() {
        let fixture = repository_root().join("tests/compile/pass/56_tagged_sum_match.jadpo");
        let analyzed = analyze_project(&fixture).expect("tagged-sum fixture should analyze");
        assert!(analyzed.syntax.diagnostics().next().is_none());
        assert!(analyzed.semantics.diagnostics.is_empty());
        assert!(analyzed.typing.diagnostics.is_empty());
        let targets = derive_target(&fixture, &analyzed).expect("tagged target should generate");
        let application = &targets[0].contents;

        assert!(application.contains("readonly tag: \"paid\""));
        assert!(application.contains("const tag = validateText(object[\"tag\"]"));
        assert!(application.contains("switch (matchSubject"));
        assert!(application.contains("const { receipt_id, paid_at } = matchSubject"));
        assert!(application
            .contains("return { tag: \"paid\", receipt_id: receipt_id, paid_at: paid_at };"));
    }

    #[test]
    fn generates_operator_precedence_with_explicit_parentheses() {
        let fixture =
            repository_root().join("tests/compile/pass/57_operators_and_precedence.jadpo");
        let analyzed = analyze_project(&fixture).expect("operator fixture should analyze");
        assert!(analyzed.typing.diagnostics.is_empty());
        let targets = derive_target(&fixture, &analyzed).expect("operator target should generate");
        let application = &targets[0].contents;

        assert!(application.contains("(left + (right * multiplier))"));
        assert!(application.contains("((!disabled) &&"));
        assert!(application.contains("((left < right) || (left === right))"));
        assert!(application.contains("(left / right)"));
        assert!(application.contains("(prefix + suffix)"));
    }

    #[test]
    fn generates_a_structured_authored_test_runner() {
        let fixture = repository_root().join("tests/compile/pass/58_authored_tests.jadpo");
        let analyzed = analyze_project(&fixture).expect("test fixture should analyze");
        let targets = derive_target(&fixture, &analyzed).expect("test target should generate");
        let application = &targets[0].contents;
        let runner = targets
            .iter()
            .find(|target| target.relative_path == "target/tests.ts")
            .expect("test entrypoint should be generated");

        assert!(application.contains("export async function runTests()"));
        assert!(application.contains("assertion failed at source bytes"));
        assert!(runner
            .contents
            .contains("import { runTests } from \"./app.ts\""));
        assert!(!runner.contents.contains("console.error(error)"));
    }

    #[test]
    fn generates_parameterised_persistence_contracts() {
        let seed = repository_root().join("examples/persistence-seed");
        let analyzed = analyze_project(&seed).expect("persistence seed should analyze");
        let targets = derive_target(&seed, &analyzed).expect("persistence target should generate");

        let postgres = targets
            .iter()
            .find(|target| target.relative_path == "persistence/entities.json")
            .expect("persistence manifest should exist");
        let runtime = targets
            .iter()
            .find(|target| target.relative_path == "target/persistence.ts")
            .expect("SQLite target should exist");
        let postgres_schema = targets
            .iter()
            .find(|target| target.relative_path == "sql/postgres/schema.sql")
            .expect("PostgreSQL schema should exist");
        assert!(postgres.contents.contains("VALUES ($1, $2)"));
        assert!(postgres.contents.contains("VALUES (?1, ?2)"));
        assert!(postgres
            .contents
            .contains("\"transaction_policy\":\"mutative_action\""));
        assert!(postgres
            .contents
            .contains("\"nested_transaction_policy\":\"reuse\""));
        assert!(postgres.contents.contains("\"identity\":\"id\""));
        assert!(postgres
            .contents
            .contains("\"unique_constraints\":[\"handle\"]"));
        assert!(postgres.contents.contains(
            "\"compound_unique_constraints\":[{\"name\":\"tenant_owner\",\"fields\":[\"tenant\",\"owner_email\"]}]"
        ));
        assert!(postgres.contents.contains("\"indexes\":[\"owner_email\"]"));
        assert!(postgres.contents.contains(
            "\"reference\":{\"entity\":\"User\",\"field\":\"id\",\"relationship\":\"owner\",\"required\":true,\"on_delete\":\"cascade\"}"
        ));
        assert!(postgres.contents.contains(
            "\"inverses\":[{\"name\":\"todos\",\"cardinality\":\"many\",\"entity\":\"Todo\",\"via\":\"Todo.owner_id\"},{\"name\":\"notes\",\"cardinality\":\"many\",\"entity\":\"Note\",\"via\":\"Note.owner_id\"},{\"name\":\"profile\",\"cardinality\":\"optional\",\"entity\":\"UserProfile\",\"via\":\"UserProfile.user_id\"}]"
        ));
        assert!(postgres
            .contents
            .contains("\"strategy\":\"bounded_batch\",\"query_count\":2"));
        assert!(postgres.contents.contains(
            "\"child\":\"PatchItem\",\"relationship\":\"reviewer_id\",\"parent\":\"User\",\"strategy\":\"bounded_parent_lookup\",\"query_count\":2"
        ));
        assert!(postgres.contents.contains(
            "\"parent\":\"User\",\"relationship\":\"profile\",\"child\":\"UserProfile\",\"strategy\":\"bounded_optional_inverse\",\"query_count\":2"
        ));
        assert!(postgres.contents.contains(
            "\"root\":\"Todo\",\"path\":[\"owner\",\"profile\"],\"leaf\":\"UserProfile\",\"strategy\":\"bounded_nested_lookup\",\"query_count\":3,\"maximum_depth\":2"
        ));
        assert!(postgres
            .contents
            .contains("\"parent_cardinality\":\"optional\""));
        assert!(postgres
            .contents
            .contains("\"strategy\":\"parent_page_join\",\"query_count\":1"));
        assert!(postgres
            .contents
            .contains("\"parent_pagination_before_join\":true"));
        assert!(postgres
            .contents
            .contains("\"strategy\":\"parent_then_bounded_children\",\"query_count\":3"));
        assert!(postgres
            .contents
            .contains("\"strategy\":\"independent_parent_page_joins\",\"query_count\":2"));
        assert!(postgres
            .contents
            .contains("\"cartesian_product_avoided\":true"));
        assert!(postgres.contents.contains("LIMIT $2 OFFSET $3"));
        assert!(postgres.contents.contains("LIMIT ?2 OFFSET ?3"));
        assert!(postgres.contents.contains(
            "SELECT \\\"id\\\", \\\"email\\\" FROM \\\"customer\\\" WHERE \\\"id\\\" = $1 LIMIT 2"
        ));
        assert!(postgres.contents.contains(
            "SELECT \\\"id\\\", \\\"email\\\" FROM \\\"customer\\\" WHERE \\\"id\\\" = ?1 LIMIT 2"
        ));
        assert!(postgres.contents.contains(
            "UPDATE \\\"customer\\\" SET \\\"email\\\" = $1 WHERE \\\"id\\\" = $2 RETURNING"
        ));
        assert!(postgres.contents.contains(
            "UPDATE \\\"account\\\" SET \\\"handle\\\" = $1, \\\"owner_email\\\" = $2 WHERE \\\"id\\\" = $3 RETURNING"
        ));
        assert!(postgres.contents.contains(
            "UPDATE \\\"patch_item\\\" SET \\\"title\\\" = CASE WHEN $1 THEN $2 ELSE \\\"title\\\" END, \\\"note\\\" = CASE WHEN $3 THEN $4 ELSE \\\"note\\\" END, \\\"marker\\\" = CASE WHEN $1 THEN $5 ELSE \\\"marker\\\" END WHERE \\\"id\\\" = $6 RETURNING"
        ));
        assert!(postgres.contents.contains("\"omission\":\"supplied_flag\""));
        assert!(postgres
            .contents
            .contains("DELETE FROM \\\"customer\\\" WHERE \\\"id\\\" = ?1 RETURNING"));
        assert!(runtime.contents.contains("${value[\"id\"]}"));
        assert!(runtime
            .contents
            .contains("query_optional_Customer_by_id(value: unknown)"));
        assert!(runtime
            .contents
            .contains("query_required_Customer_by_id(value: unknown)"));
        assert!(runtime
            .contents
            .contains("class PersistenceFault extends Error"));
        assert!(runtime
            .contents
            .contains("new SQL({ url: Bun.env.DATABASE_URL!, prepare: false })"));
        assert!(runtime.contents.contains("details.errno"));
        assert!(runtime
            .contents
            .contains("persistenceAsync(\"query.Customer.id\""));
        assert!(runtime
            .contents
            .contains("update_required_Customer_by_id_set_email"));
        assert!(runtime
            .contents
            .contains("update_required_Account_by_id_set_handle_and_owner_email"));
        assert!(runtime
            .contents
            .contains("update_required_PatchItem_by_id_patch_title_and_note_set_marker"));
        assert!(runtime
            .contents
            .contains("CASE WHEN ${hasOwn(patchValue, \"title\")}"));
        assert!(runtime
            .contents
            .contains("CASE WHEN ?1 THEN ?2 ELSE \\\"title\\\" END"));
        assert!(targets[0].contents.contains("note?: string | null;"));
        assert!(targets[0].contents.contains(
            "hasOwn(object, \"note\") ? { note: object[\"note\"] === null ? null : validateText"
        ));
        assert!(runtime
            .contents
            .contains("\"account_tenant_owner_unique\":\"Account.tenant_owner\""));
        assert!(runtime
            .contents
            .contains("UNIQUE constraint failed: account.tenant, account.owner_email"));
        assert!(runtime.contents.contains("delete_required_Customer_by_id"));
        assert!(runtime
            .contents
            .contains("query_required_User_with_todos_by_id_order_by_id_asc"));
        assert!(runtime
            .contents
            .contains("query_required_PatchItem_with_reviewer_id_optional_by_id"));
        assert!(runtime
            .contents
            .contains("query_required_User_with_profile_optional_by_id"));
        assert!(runtime
            .contents
            .contains("return { parent, profile: related };"));
        assert!(runtime
            .contents
            .contains("query_required_Todo_with_owner_then_profile_optional_by_id"));
        assert!(runtime
            .contents
            .contains("return { parent: root, owner: { parent: owner, profile: leaf } };"));
        assert!(targets[0]
            .contents
            .contains("persistence.query_required_User_with_profile_optional_by_id(input.id)"));
        assert!(targets[0].contents.contains(
            "persistence.query_required_Todo_with_owner_then_profile_optional_by_id(input.id)"
        ));
        assert!(runtime
            .contents
            .contains("return { parent, reviewer_id: related };"));
        assert!(runtime.contents.contains(
            "query_many_User_with_todos_by_group_order_by_id_asc_include_order_by_id_asc_paginated"
        ));
        assert!(runtime.contents.contains(
            "query_many_Todo_by_owner_id_order_by_id_asc_paginated(value: unknown, limit: unknown, offset: unknown)"
        ));
        assert!(runtime
            .contents
            .contains("return { parent, todos: children };"));
        assert!(runtime.contents.contains("postgres.begin(async tx"));
        assert!(runtime.contents.contains("sqlite!.transaction"));
        assert!(runtime.contents.contains("BEGIN IMMEDIATE"));
        assert!(runtime
            .contents
            .contains("if (transactional) return work(client)"));
        assert!(runtime.contents.contains("PRAGMA foreign_keys = ON"));
        assert!(runtime
            .contents
            .contains("CONSTRAINT \\\"account_identity\\\" PRIMARY KEY (\\\"id\\\")"));
        assert!(runtime
            .contents
            .contains("CONSTRAINT \\\"account_handle_unique\\\" UNIQUE (\\\"handle\\\")"));
        assert!(runtime.contents.contains(
            "CONSTRAINT \\\"account_tenant_owner_unique\\\" UNIQUE (\\\"tenant\\\", \\\"owner_email\\\")"
        ));
        assert!(runtime
            .contents
            .contains("CREATE INDEX IF NOT EXISTS \\\"account_owner_email_idx\\\""));
        assert!(
            postgres_schema
                .contents
                .find("CREATE TABLE IF NOT EXISTS \"user\"")
                < postgres_schema
                    .contents
                    .find("CREATE TABLE IF NOT EXISTS \"todo\"")
        );
        assert!(postgres_schema.contents.contains(
            "CONSTRAINT \"todo_owner_id_fk\" FOREIGN KEY (\"owner_id\") REFERENCES \"user\" (\"id\") ON DELETE CASCADE"
        ));
        assert!(postgres_schema.contents.contains(
            "CREATE INDEX IF NOT EXISTS \"todo_owner_id_idx\" ON \"todo\" (\"owner_id\")"
        ));
        assert!(targets[0]
            .contents
            .contains("validate_Customer(await persistence.create_Customer"));
        assert!(targets[0]
            .contents
            .contains("await persistence.query_optional_Customer_by_id(input.id)"));
        assert!(targets[0]
            .contents
            .contains("await persistence.query_required_Customer_by_id(input.id)"));
        assert!(targets[0]
            .contents
            .contains("new DomainFailure(\"CustomerNotFound\""));
        assert!(targets[0]
            .contents
            .contains("new DomainFailure(\"CustomerMutationConflict\""));
        assert!(targets[0].contents.contains(
            "persistence.update_required_Account_by_id_set_handle_and_owner_email(input.id, input.handle, input.owner_email)"
        ));
        assert!(targets[0]
            .contents
            .contains("error.constraint === \"Account.handle\""));
        assert!(targets[0]
            .contents
            .contains("error.constraint === \"Account.tenant_owner\""));
        assert!(targets[0]
            .contents
            .contains("error.kind === \"constraint\""));
        assert!(targets[0]
            .contents
            .contains("async function create_atomic_pair(input: CreateAtomicPair, __persistence"));
        assert!(targets[0]
            .contents
            .contains("return __persistence.transaction(async persistence =>"));
        assert!(targets[0]
            .contents
            .contains("async function find_customer(input: FindCustomer, __persistence"));
        assert!(!targets[0].contents.contains(
            "async function find_customer(input: FindCustomer, __persistence: typeof rootPersistence = rootPersistence): Promise<Customer | null> {\n  return __persistence.transaction"
        ));
    }

    #[test]
    fn rejects_external_runtime_dependencies_and_manifests() {
        let external = vec![GeneratedArtifact {
            relative_path: "target/app.ts",
            contents: "import \"third-party-package\";\n".to_owned(),
        }];
        let diagnostic = validate_runtime_dependency_contract(&external)
            .expect_err("bare package imports must be rejected");
        assert_eq!(diagnostic.code, "JADPO_TARGET_EXTERNAL_MODULE");

        let manifest = vec![GeneratedArtifact {
            relative_path: "package.json",
            contents: "{}\n".to_owned(),
        }];
        let diagnostic = validate_runtime_dependency_contract(&manifest)
            .expect_err("dependency manifests must be rejected");
        assert_eq!(diagnostic.code, "JADPO_TARGET_DEPENDENCY_MANIFEST");
    }
}
