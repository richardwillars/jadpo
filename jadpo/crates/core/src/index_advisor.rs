use crate::{
    analyze_project, register_schema_additions, validate_schema_identities, AnalyzedProject,
};
use jadpo_diagnostics::Diagnostic;
use jadpo_syntax::{
    Block, Declaration, Expression, FieldDeclaration, PersistenceModifier, QueryCardinality,
    QueryExpression, RecordKind, Statement,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
struct IndexRecommendation {
    entity: String,
    field: String,
    reasons: BTreeSet<(String, &'static str)>,
}

pub fn index_recommendations_json(analyzed: &AnalyzedProject) -> String {
    let recommendations = derive_index_recommendations(analyzed)
        .into_iter()
        .map(|recommendation| {
            let reasons = recommendation
                .reasons
                .iter()
                .map(|(callable, role)| {
                    format!(
                        "{{\"callable\":{},\"role\":{}}}",
                        json_string(callable),
                        json_string(role)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            let path = format!("{}.{}", recommendation.entity, recommendation.field);
            format!(
                "{{\"id\":{},\"path\":{},\"entity\":{},\"field\":{},\"source_modifier\":\"index\",\"accepted\":false,\"reasons\":[{reasons}]}}",
                json_string(&format!("index-rec-v1/{path}")),
                json_string(&path),
                json_string(&recommendation.entity),
                json_string(&recommendation.field)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema_version\":1,\"kind\":\"index_recommendations\",\"basis\":\"static_query_shape\",\"automatic_changes\":false,\"recommendations\":[{recommendations}]}}\n"
    )
}

pub fn index_recommendation_count(analyzed: &AnalyzedProject) -> usize {
    derive_index_recommendations(analyzed).len()
}

pub fn accept_index_recommendation(
    project: &Path,
    analyzed: &AnalyzedProject,
    path: &str,
) -> Result<(PathBuf, PathBuf), Diagnostic> {
    validate_schema_identities(project, analyzed)?.ok_or_else(|| {
        Diagnostic::error("INDEX_ACCEPT_REGISTRY_MISSING")
            .with_note("run `jadpo schema init <project>` before accepting an index")
    })?;
    let recommendation = derive_index_recommendations(analyzed)
        .into_iter()
        .find(|recommendation| {
            format!("{}.{}", recommendation.entity, recommendation.field) == path
        })
        .ok_or_else(|| {
            Diagnostic::error("INDEX_RECOMMENDATION_UNKNOWN").with_note(
                "run `jadpo schema index-recommend <project>` to inspect current evidence",
            )
        })?;

    let (source_path, field) = find_field(analyzed, &recommendation.entity, &recommendation.field)
        .ok_or_else(|| Diagnostic::error("INDEX_RECOMMENDATION_FIELD_MISSING"))?;
    let original = fs::read_to_string(&source_path)
        .map_err(|_error| Diagnostic::error("INDEX_ACCEPT_SOURCE_READ_FAILED"))?;
    let insertion = index_insertion_offset(&original, &field)?;
    let mut updated = original.clone();
    updated.insert_str(insertion.0, insertion.1);
    fs::write(&source_path, &updated)
        .map_err(|_error| Diagnostic::error("INDEX_ACCEPT_SOURCE_WRITE_FAILED"))?;

    let result = (|| {
        let updated_project = analyze_project(project)?;
        let diagnostics = updated_project
            .syntax
            .diagnostics()
            .chain(&updated_project.semantics.diagnostics)
            .chain(&updated_project.typing.diagnostics)
            .chain(&updated_project.failures.diagnostics)
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>();
        if !diagnostics.is_empty() {
            return Err(Diagnostic::error("INDEX_ACCEPT_CHECK_FAILED"));
        }
        let (registry, added) = register_schema_additions(project, &updated_project)?;
        if added != 1 {
            return Err(Diagnostic::error("INDEX_ACCEPT_IDENTITY_COUNT"));
        }
        Ok(registry)
    })();

    match result {
        Ok(registry) => Ok((source_path, registry)),
        Err(diagnostic) => {
            fs::write(&source_path, original)
                .map_err(|_error| Diagnostic::error("INDEX_ACCEPT_ROLLBACK_FAILED"))?;
            Err(diagnostic)
        }
    }
}

fn derive_index_recommendations(analyzed: &AnalyzedProject) -> Vec<IndexRecommendation> {
    let entities = analyzed
        .syntax
        .sources
        .iter()
        .flat_map(|source| &source.file.declarations)
        .filter_map(|declaration| match declaration {
            Declaration::Record(record) if record.kind == RecordKind::Entity => {
                Some((record.name.text.as_str(), record))
            }
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let mut recommendations = BTreeMap::<(String, String), IndexRecommendation>::new();
    for source in &analyzed.syntax.sources {
        for declaration in &source.file.declarations {
            let Declaration::Callable(callable) = declaration else {
                continue;
            };
            let mut queries = Vec::new();
            collect_queries(&callable.body, &mut queries);
            for query in queries {
                let Some(entity) = query.target.path.last().map(|name| name.text.as_str()) else {
                    continue;
                };
                add_candidate(
                    &mut recommendations,
                    &entities,
                    entity,
                    &query.field.text,
                    &callable.name.text,
                    "predicate",
                );
                if query.cardinality == QueryCardinality::Many {
                    if let Some(order) = &query.order {
                        add_candidate(
                            &mut recommendations,
                            &entities,
                            entity,
                            &order.field.text,
                            &callable.name.text,
                            "ordering",
                        );
                    }
                }
            }
        }
    }
    recommendations.into_values().collect()
}

fn add_candidate(
    recommendations: &mut BTreeMap<(String, String), IndexRecommendation>,
    entities: &BTreeMap<&str, &jadpo_syntax::RecordDeclaration>,
    entity: &str,
    field_name: &str,
    callable: &str,
    role: &'static str,
) {
    let Some(field) = entities.get(entity).and_then(|record| {
        record
            .fields
            .iter()
            .find(|field| field.name.text == field_name)
    }) else {
        return;
    };
    if field.reference.is_some()
        || field.persistence.iter().any(|modifier| {
            matches!(
                modifier,
                PersistenceModifier::Identity
                    | PersistenceModifier::Unique
                    | PersistenceModifier::Index
            )
        })
    {
        return;
    }
    recommendations
        .entry((entity.to_owned(), field_name.to_owned()))
        .or_insert_with(|| IndexRecommendation {
            entity: entity.to_owned(),
            field: field_name.to_owned(),
            reasons: BTreeSet::new(),
        })
        .reasons
        .insert((callable.to_owned(), role));
}

fn collect_queries<'a>(block: &'a Block, queries: &mut Vec<&'a QueryExpression>) {
    for statement in &block.statements {
        match statement {
            Statement::Binding(statement) => collect_expression(&statement.value, queries),
            Statement::Assignment(statement) => collect_expression(&statement.value, queries),
            Statement::Return(statement) => collect_expression(&statement.value, queries),
            Statement::Reject(statement) => {
                for field in &statement.values {
                    collect_expression(&field.value, queries);
                }
            }
            Statement::If(statement) => {
                collect_expression(&statement.condition, queries);
                collect_queries(&statement.then_block, queries);
                if let Some(block) = &statement.else_block {
                    collect_queries(block, queries);
                }
            }
            Statement::Match(statement) => {
                collect_expression(&statement.subject, queries);
                for arm in &statement.arms {
                    collect_queries(&arm.body, queries);
                }
            }
            Statement::Assert(statement) => collect_expression(&statement.condition, queries),
            Statement::Unsupported(_) => {}
        }
    }
}

fn collect_expression<'a>(expression: &'a Expression, queries: &mut Vec<&'a QueryExpression>) {
    match expression {
        Expression::Query(query) => {
            queries.push(query);
            collect_expression(&query.value, queries);
            if let Some(pagination) = &query.pagination {
                collect_expression(&pagination.limit, queries);
                collect_expression(&pagination.offset, queries);
            }
            for include in &query.includes {
                collect_expression(&include.pagination.limit, queries);
                collect_expression(&include.pagination.offset, queries);
            }
        }
        Expression::Invocation(invocation) => {
            for argument in &invocation.arguments {
                collect_expression(argument, queries);
            }
        }
        Expression::Construction(construction) => {
            for field in &construction.fields {
                collect_expression(&field.value, queries);
            }
        }
        Expression::Create(create) => {
            for field in &create.fields {
                collect_expression(&field.value, queries);
            }
        }
        Expression::Update(update) => {
            collect_expression(&update.value, queries);
            for field in &update.changes {
                collect_expression(&field.value, queries);
            }
            for conditional in &update.conditional_changes {
                collect_expression(&conditional.change.value, queries);
            }
        }
        Expression::Delete(delete) => collect_expression(&delete.value, queries),
        Expression::Binary(binary) => {
            collect_expression(&binary.left, queries);
            collect_expression(&binary.right, queries);
        }
        Expression::Unary(unary) => collect_expression(&unary.value, queries),
        Expression::Grouped(grouped) => collect_expression(&grouped.value, queries),
        Expression::Attempt(attempt) => collect_expression(&attempt.value, queries),
        Expression::Literal(_) | Expression::Name(_) | Expression::Missing(_) => {}
    }
}

fn find_field(
    analyzed: &AnalyzedProject,
    entity: &str,
    field: &str,
) -> Option<(PathBuf, FieldDeclaration)> {
    analyzed.syntax.sources.iter().find_map(|source| {
        source.file.declarations.iter().find_map(|declaration| {
            let Declaration::Record(record) = declaration else {
                return None;
            };
            if record.kind != RecordKind::Entity || record.name.text != entity {
                return None;
            }
            record
                .fields
                .iter()
                .find(|candidate| candidate.name.text == field)
                .cloned()
                .map(|field| (PathBuf::from(&source.source_name), field))
        })
    })
}

fn index_insertion_offset<'a>(
    source: &'a str,
    field: &FieldDeclaration,
) -> Result<(usize, &'a str), Diagnostic> {
    if let Some(reference) = &field.reference {
        return Ok((reference.range.start, "index "));
    }
    if field.optional {
        let field_source = source
            .get(field.range.start..field.range.end)
            .ok_or_else(|| Diagnostic::error("INDEX_ACCEPT_RANGE_INVALID"))?;
        let relative = field_source
            .rfind("optional")
            .ok_or_else(|| Diagnostic::error("INDEX_ACCEPT_RANGE_INVALID"))?;
        return Ok((field.range.start + relative, "index "));
    }
    Ok((field.range.end, " index"))
}

fn json_string(value: &str) -> String {
    let mut output = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
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
    output.push('"');
    output
}

#[cfg(test)]
mod tests {
    use super::{accept_index_recommendation, index_recommendations_json};
    use crate::{analyze_project, initialize_schema_identities, validate_schema_identities};
    use std::fs;

    #[test]
    fn recommends_query_fields_and_accepts_one_explicitly() {
        let root = std::env::temp_dir().join(format!("jadpo-index-advisor-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).expect("stale test directory should be removable");
        }
        fs::create_dir_all(&root).expect("test project should be created");
        fs::write(
            root.join("app.jadpo"),
            "entity Item { id: Uuid identity category: Text }\ninput ListItems { category: Item.category }\naction list_items(input: ListItems) -> List<Item> { return attempt query many Item { where: category == input.category order_by: id asc } }\n",
        )
        .expect("test source should be written");
        let analyzed = analyze_project(&root).expect("test project should analyze");
        assert!(analyzed.semantics.diagnostics.is_empty());
        initialize_schema_identities(&root, &analyzed).expect("schema registry should initialize");

        let recommendations = index_recommendations_json(&analyzed);
        assert!(recommendations.contains("\"path\":\"Item.category\""));
        assert!(recommendations.contains("\"role\":\"predicate\""));
        assert!(!recommendations.contains("\"path\":\"Item.id\""));

        let (source, registry) = accept_index_recommendation(&root, &analyzed, "Item.category")
            .expect("explicit acceptance should update source and identity registry");
        let source = fs::read_to_string(source).expect("updated source should be readable");
        assert!(source.contains("category: Text index"));
        let registry = fs::read_to_string(registry).expect("updated registry should be readable");
        assert!(registry.contains("\"id\":\"schema-v1/index/Item.category\""));

        let accepted = analyze_project(&root).expect("accepted project should analyze");
        validate_schema_identities(&root, &accepted)
            .expect("accepted source and registry should remain aligned");
        let recommendations = index_recommendations_json(&accepted);
        assert!(!recommendations.contains("\"path\":\"Item.category\""));
        assert_eq!(
            accept_index_recommendation(&root, &accepted, "Item.category")
                .expect_err("an accepted recommendation must not be accepted again")
                .code,
            "INDEX_RECOMMENDATION_UNKNOWN"
        );
        fs::remove_dir_all(root).expect("test project should be removable");
    }
}
