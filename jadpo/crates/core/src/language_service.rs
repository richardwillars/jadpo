use crate::AnalyzedProject;
use jadpo_semantic::{NodeKind, SemanticNode};
use jadpo_syntax::{
    Block, Declaration, Expression, FieldInitialiser, MatchPattern, NameExpression, ParsedSyntax,
    Statement, TextRange, Token, TokenKind,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanguageSymbol {
    pub key: String,
    pub name: String,
    pub kind: String,
    pub detail: String,
    pub type_name: Option<String>,
    pub source: String,
    pub range: TextRange,
    pub container: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanguageOccurrence {
    pub key: String,
    pub source: String,
    pub range: TextRange,
    pub declaration: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LocalDefinition {
    symbol: LanguageSymbol,
    scope: TextRange,
    available_from: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LanguageIndex {
    pub symbols: Vec<LanguageSymbol>,
    pub occurrences: Vec<LanguageOccurrence>,
    locals: Vec<LocalDefinition>,
}

impl LanguageIndex {
    pub fn build(project: &AnalyzedProject) -> Self {
        let mut symbols = project
            .semantics
            .nodes
            .iter()
            .filter(|node| !matches!(node.kind, NodeKind::PreludeType | NodeKind::StandardFailure))
            .filter(|node| !node.name.starts_with("__jadpo_"))
            .map(|node| semantic_symbol(node, project))
            .collect::<Vec<_>>();
        let mut locals = Vec::new();
        for source in &project.syntax.sources {
            collect_local_definitions(source, project, &mut locals);
        }
        symbols.extend(locals.iter().map(|definition| definition.symbol.clone()));
        symbols.sort_by(|left, right| {
            (&left.source, left.range.start, &left.key).cmp(&(
                &right.source,
                right.range.start,
                &right.key,
            ))
        });

        let globals = symbols
            .iter()
            .filter(|symbol| !symbol.key.starts_with("local:"))
            .map(|symbol| (symbol.key.clone(), symbol.clone()))
            .collect::<BTreeMap<_, _>>();
        let declaration_ranges = symbols
            .iter()
            .map(|symbol| {
                (
                    (symbol.source.clone(), symbol.range.start, symbol.range.end),
                    symbol.key.clone(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut explicit_references = BTreeMap::new();
        for source in &project.syntax.sources {
            collect_explicit_references(source, &globals, &mut explicit_references);
        }
        let mut occurrences = Vec::new();
        for source in &project.syntax.sources {
            collect_occurrences(
                source,
                project,
                &globals,
                &locals,
                &declaration_ranges,
                &explicit_references,
                &mut occurrences,
            );
        }
        occurrences.sort_by(|left, right| {
            (&left.source, left.range.start, left.range.end, &left.key).cmp(&(
                &right.source,
                right.range.start,
                right.range.end,
                &right.key,
            ))
        });
        occurrences.dedup();

        Self {
            symbols,
            occurrences,
            locals,
        }
    }

    pub fn symbol(&self, key: &str) -> Option<&LanguageSymbol> {
        self.symbols.iter().find(|symbol| symbol.key == key)
    }

    pub fn symbol_at(&self, source: &str, offset: usize) -> Option<&LanguageSymbol> {
        self.occurrences
            .iter()
            .filter(|occurrence| {
                occurrence.source == source
                    && occurrence.range.start <= offset
                    && offset < occurrence.range.end
            })
            .min_by_key(|occurrence| occurrence.range.end - occurrence.range.start)
            .and_then(|occurrence| self.symbol(&occurrence.key))
    }

    pub fn occurrences_for<'index>(
        &'index self,
        key: &'index str,
    ) -> impl Iterator<Item = &'index LanguageOccurrence> + 'index {
        self.occurrences
            .iter()
            .filter(move |occurrence| occurrence.key == key)
    }

    /// Reject local renames that could capture a binding in an overlapping scope.
    /// Names in separate functions or sibling blocks do not conflict.
    pub fn local_rename_conflicts(&self, key: &str, new_name: &str) -> bool {
        let Some(selected) = self.locals.iter().find(|local| local.symbol.key == key) else {
            return false;
        };
        if selected.symbol.name == new_name {
            return false;
        }
        self.locals.iter().any(|candidate| {
            candidate.symbol.key != key
                && candidate.symbol.source == selected.symbol.source
                && candidate.symbol.name == new_name
                && candidate.scope.start < selected.scope.end
                && selected.scope.start < candidate.scope.end
        })
    }

    pub fn locals_at(&self, source: &str, offset: usize) -> Vec<&LanguageSymbol> {
        let mut seen = BTreeSet::new();
        let mut definitions = self
            .locals
            .iter()
            .filter(|definition| {
                definition.symbol.source == source
                    && definition.scope.start <= offset
                    && offset <= definition.scope.end
                    && definition.available_from <= offset
            })
            .collect::<Vec<_>>();
        definitions.sort_by_key(|definition| std::cmp::Reverse(definition.symbol.range.start));
        definitions
            .into_iter()
            .filter(|definition| seen.insert(definition.symbol.name.clone()))
            .map(|definition| &definition.symbol)
            .collect()
    }

    pub fn inferred_type_at<'project>(
        &self,
        project: &'project AnalyzedProject,
        source: &str,
        offset: usize,
    ) -> Option<&'project str> {
        project
            .typing
            .expressions
            .iter()
            .filter(|expression| {
                expression.source == source
                    && expression.range.start <= offset
                    && offset < expression.range.end
            })
            .min_by_key(|expression| expression.range.end - expression.range.start)
            .map(|expression| expression.type_name.as_str())
    }
}

fn semantic_symbol(node: &SemanticNode, project: &AnalyzedProject) -> LanguageSymbol {
    let parent = project
        .semantics
        .refinements
        .iter()
        .find(|edge| edge.refined == node.id)
        .map(|edge| &project.semantics.nodes[edge.parent.0 as usize].name);
    let detail = match parent {
        Some(parent) => format!("{} {}: {parent}", node.kind.as_str(), node.name),
        None => format!("{} {}", node.kind.as_str(), node.name),
    };
    let container = node
        .name
        .rsplit_once('.')
        .map(|(container, _)| container.to_owned());
    LanguageSymbol {
        key: node.name.clone(),
        name: node
            .name
            .rsplit('.')
            .next()
            .unwrap_or(&node.name)
            .to_owned(),
        kind: node.kind.as_str().to_owned(),
        detail,
        type_name: parent.cloned(),
        source: node.source.clone(),
        range: node.range,
        container,
    }
}

fn collect_local_definitions(
    source: &ParsedSyntax,
    project: &AnalyzedProject,
    output: &mut Vec<LocalDefinition>,
) {
    for declaration in &source.file.declarations {
        match declaration {
            Declaration::Callable(callable) => {
                for parameter in &callable.parameters {
                    push_local(
                        source,
                        &parameter.name.text,
                        "parameter",
                        parameter.name.range,
                        callable.body.range,
                        callable.body.range.start,
                        Some(callable.name.text.clone()),
                        Some(
                            parameter
                                .parameter_type
                                .path
                                .iter()
                                .map(|name| name.text.as_str())
                                .collect::<Vec<_>>()
                                .join("."),
                        ),
                        output,
                    );
                }
                collect_block_locals(
                    source,
                    &callable.body,
                    Some(callable.name.text.clone()),
                    project,
                    output,
                );
            }
            Declaration::Test(test) => {
                collect_block_locals(
                    source,
                    &test.body,
                    Some(test.name.text.clone()),
                    project,
                    output,
                );
            }
            _ => {}
        }
    }
}

fn collect_block_locals(
    source: &ParsedSyntax,
    block: &Block,
    container: Option<String>,
    project: &AnalyzedProject,
    output: &mut Vec<LocalDefinition>,
) {
    for statement in &block.statements {
        match statement {
            Statement::Binding(binding) => {
                push_local(
                    source,
                    &binding.name.text,
                    "variable",
                    binding.name.range,
                    block.range,
                    binding.range.end,
                    container.clone(),
                    inferred_expression_type(project, source, binding.value.range()),
                    output,
                );
                collect_outcome_match_locals(
                    source,
                    &binding.value,
                    container.clone(),
                    project,
                    output,
                );
            }
            Statement::Assignment(statement) => collect_outcome_match_locals(
                source,
                &statement.value,
                container.clone(),
                project,
                output,
            ),
            Statement::Return(statement) => collect_outcome_match_locals(
                source,
                &statement.value,
                container.clone(),
                project,
                output,
            ),
            Statement::If(statement) => {
                collect_block_locals(
                    source,
                    &statement.then_block,
                    container.clone(),
                    project,
                    output,
                );
                if let Some(else_block) = &statement.else_block {
                    collect_block_locals(source, else_block, container.clone(), project, output);
                }
            }
            Statement::Match(statement) => {
                for arm in &statement.arms {
                    match &arm.pattern {
                        MatchPattern::OptionalSome(pattern) => push_local(
                            source,
                            &pattern.binding.text,
                            "variable",
                            pattern.binding.range,
                            arm.body.range,
                            arm.body.range.start,
                            container.clone(),
                            inferred_expression_type(project, source, statement.subject.range())
                                .map(|name| name.trim_end_matches('?').to_owned()),
                            output,
                        ),
                        MatchPattern::Variant(pattern) => {
                            for binding in &pattern.bindings {
                                push_local(
                                    source,
                                    &binding.text,
                                    "variable",
                                    binding.range,
                                    arm.body.range,
                                    arm.body.range.start,
                                    container.clone(),
                                    Some(format!(
                                        "{}.{}",
                                        joined_name(&pattern.target),
                                        binding.text
                                    )),
                                    output,
                                );
                            }
                        }
                        _ => {}
                    }
                    collect_block_locals(source, &arm.body, container.clone(), project, output);
                }
            }
            _ => {}
        }
    }
}

fn collect_outcome_match_locals(
    source: &ParsedSyntax,
    expression: &Expression,
    container: Option<String>,
    project: &AnalyzedProject,
    output: &mut Vec<LocalDefinition>,
) {
    let Expression::OutcomeMatch(outcome) = expression else {
        return;
    };
    let subject_type = inferred_expression_type(project, source, outcome.subject.range());
    for arm in &outcome.arms {
        if let jadpo_syntax::OutcomeMatchPattern::Success(binding) = &arm.pattern {
            push_local(
                source,
                &binding.text,
                "variable",
                binding.range,
                arm.body.range(),
                arm.body.range().start,
                container.clone(),
                subject_type.clone(),
                output,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn push_local(
    source: &ParsedSyntax,
    name: &str,
    kind: &str,
    range: TextRange,
    scope: TextRange,
    available_from: usize,
    container: Option<String>,
    type_name: Option<String>,
    output: &mut Vec<LocalDefinition>,
) {
    let key = format!("local:{}:{}:{name}", source.source_name, range.start);
    output.push(LocalDefinition {
        symbol: LanguageSymbol {
            key,
            name: name.to_owned(),
            kind: kind.to_owned(),
            detail: type_name.as_ref().map_or_else(
                || format!("{kind} {name}"),
                |type_name| format!("{kind} {name}: {type_name}"),
            ),
            type_name,
            source: source.source_name.clone(),
            range,
            container,
        },
        scope,
        available_from,
    });
}

fn inferred_expression_type(
    project: &AnalyzedProject,
    source: &ParsedSyntax,
    range: TextRange,
) -> Option<String> {
    project
        .typing
        .expressions
        .iter()
        .find(|expression| expression.source == source.source_name && expression.range == range)
        .map(|expression| expression.type_name.clone())
}

fn collect_occurrences(
    source: &ParsedSyntax,
    project: &AnalyzedProject,
    globals: &BTreeMap<String, LanguageSymbol>,
    locals: &[LocalDefinition],
    declarations: &BTreeMap<(String, usize, usize), String>,
    explicit_references: &BTreeMap<(String, usize, usize), String>,
    output: &mut Vec<LanguageOccurrence>,
) {
    let significant = source
        .tokens
        .iter()
        .enumerate()
        .filter(|(_, token)| !token.kind.is_trivia() && token.kind != TokenKind::Eof)
        .collect::<Vec<_>>();
    for (position, (_, token)) in significant.iter().enumerate() {
        if !is_name_token(token, source) {
            continue;
        }
        let declaration_key = declarations.get(&(
            source.source_name.clone(),
            token.range.start,
            token.range.end,
        ));
        let explicit_key = explicit_references.get(&(
            source.source_name.clone(),
            token.range.start,
            token.range.end,
        ));
        let key = declaration_key
            .cloned()
            .or_else(|| explicit_key.cloned())
            .or_else(|| resolve_token(source, project, &significant, position, globals, locals));
        if let Some(key) = key {
            output.push(LanguageOccurrence {
                key,
                source: source.source_name.clone(),
                range: token.range,
                declaration: declaration_key.is_some(),
            });
        }
    }
}

fn collect_explicit_references(
    source: &ParsedSyntax,
    globals: &BTreeMap<String, LanguageSymbol>,
    output: &mut BTreeMap<(String, usize, usize), String>,
) {
    for persistence in &source.file.persistence {
        insert_explicit(
            source,
            persistence.target.range,
            persistence.target.text.clone(),
            globals,
            output,
        );
        for field in persistence
            .identities
            .iter()
            .chain(&persistence.uniques)
            .chain(&persistence.indexes)
        {
            insert_explicit(
                source,
                field.range,
                format!("{}.{}", persistence.target.text, field.text),
                globals,
                output,
            );
        }
        for constraint in &persistence.constraints {
            for field in &constraint.fields {
                insert_explicit(
                    source,
                    field.range,
                    format!("{}.{}", persistence.target.text, field.text),
                    globals,
                    output,
                );
            }
        }
        for reference in &persistence.references {
            insert_explicit(
                source,
                reference.field.range,
                format!("{}.{}", persistence.target.text, reference.field.text),
                globals,
                output,
            );
        }
    }
    for declaration in &source.file.declarations {
        match declaration {
            Declaration::Record(record) => {
                for constraint in &record.persistence_constraints {
                    for field in &constraint.fields {
                        insert_explicit(
                            source,
                            field.range,
                            format!("{}.{}", record.name.text, field.text),
                            globals,
                            output,
                        );
                    }
                }
            }
            Declaration::Callable(callable) => {
                collect_block_references(source, &callable.body, globals, output);
            }
            Declaration::Test(test) => {
                if let Some(fixture) = &test.fixture {
                    insert_explicit(source, fixture.range, fixture.text.clone(), globals, output);
                }
                collect_block_references(source, &test.body, globals, output);
            }
            Declaration::Fixture(fixture) => {
                if let Some(clock) = &fixture.clock {
                    collect_expression_references(source, clock, globals, output);
                }
                if let Some(configuration) = &fixture.configuration {
                    for item in configuration {
                        collect_expression_references(source, &item.value, globals, output);
                    }
                }
            }
            Declaration::Job(job) => {
                if let Some(run) = &job.run {
                    collect_name_expression(source, &run.callee, globals, output);
                    for argument in &run.arguments {
                        collect_expression_references(source, argument, globals, output);
                    }
                }
            }
            Declaration::Route(route) => {
                if let Some(run) = &route.run {
                    collect_name_expression(source, &run.callee, globals, output);
                    for argument in &run.arguments {
                        collect_expression_references(source, argument, globals, output);
                    }
                    for argument in &run.named_arguments {
                        collect_expression_references(source, &argument.value, globals, output);
                    }
                }
            }
            _ => {}
        }
    }
}

fn collect_block_references(
    source: &ParsedSyntax,
    block: &Block,
    globals: &BTreeMap<String, LanguageSymbol>,
    output: &mut BTreeMap<(String, usize, usize), String>,
) {
    for statement in &block.statements {
        match statement {
            Statement::Binding(statement) => {
                collect_expression_references(source, &statement.value, globals, output)
            }
            Statement::Assignment(statement) => {
                collect_expression_references(source, &statement.value, globals, output)
            }
            Statement::Return(statement) => {
                collect_expression_references(source, &statement.value, globals, output)
            }
            Statement::Reject(statement) => {
                collect_initialisers(
                    source,
                    &statement.failure.text,
                    &statement.values,
                    globals,
                    output,
                );
            }
            Statement::If(statement) => {
                collect_expression_references(source, &statement.condition, globals, output);
                collect_block_references(source, &statement.then_block, globals, output);
                if let Some(else_block) = &statement.else_block {
                    collect_block_references(source, else_block, globals, output);
                }
            }
            Statement::Match(statement) => {
                collect_expression_references(source, &statement.subject, globals, output);
                for arm in &statement.arms {
                    collect_block_references(source, &arm.body, globals, output);
                }
            }
            Statement::Assert(statement) => {
                collect_expression_references(source, &statement.condition, globals, output)
            }
            Statement::AdvanceClock(statement) => {
                collect_expression_references(source, &statement.duration, globals, output)
            }
            Statement::Unsupported(_) => {}
        }
    }
}

fn collect_expression_references(
    source: &ParsedSyntax,
    expression: &Expression,
    globals: &BTreeMap<String, LanguageSymbol>,
    output: &mut BTreeMap<(String, usize, usize), String>,
) {
    match expression {
        Expression::Literal(_) | Expression::Missing(_) => {}
        Expression::Name(name) => collect_name_expression(source, name, globals, output),
        Expression::Invocation(invocation) => {
            collect_name_expression(source, &invocation.callee, globals, output);
            for argument in &invocation.arguments {
                collect_expression_references(source, argument, globals, output);
            }
            for argument in &invocation.named_arguments {
                collect_expression_references(source, &argument.value, globals, output);
            }
        }
        Expression::TestCall(call) => {
            collect_name_expression(source, &call.invocation.callee, globals, output);
            for argument in &call.invocation.arguments {
                collect_expression_references(source, argument, globals, output);
            }
            for argument in &call.invocation.named_arguments {
                collect_expression_references(source, &argument.value, globals, output);
            }
        }
        Expression::Construction(construction) => {
            collect_name_expression(source, &construction.target, globals, output);
            collect_initialisers(
                source,
                &joined_name(&construction.target),
                &construction.fields,
                globals,
                output,
            );
        }
        Expression::Object(object) => {
            collect_initialisers(source, "Object", &object.fields, globals, output)
        }
        Expression::Create(create) => {
            collect_name_expression(source, &create.target, globals, output);
            let target = joined_name(&create.target);
            collect_initialisers(source, &target, &create.fields, globals, output);
            for conflict in &create.conflicts {
                collect_block_references(
                    source,
                    &Block {
                        statements: vec![Statement::Reject(conflict.rejection.clone())],
                        range: conflict.rejection.range,
                    },
                    globals,
                    output,
                );
            }
        }
        Expression::Query(query) => {
            collect_name_expression(source, &query.target, globals, output);
            let target = joined_name(&query.target);
            insert_explicit(
                source,
                query.field.range,
                format!("{target}.{}", query.field.text),
                globals,
                output,
            );
            collect_expression_references(source, &query.value, globals, output);
            if let Some(page) = &query.page {
                for predicate in &page.predicates {
                    insert_explicit(
                        source,
                        predicate.field.range,
                        format!("{target}.{}", predicate.field.text),
                        globals,
                        output,
                    );
                    collect_expression_references(source, &predicate.value, globals, output);
                }
                for order in &page.order {
                    insert_explicit(
                        source,
                        order.field.range,
                        format!("{target}.{}", order.field.text),
                        globals,
                        output,
                    );
                }
                for reference in [&page.result, &page.projection, &page.cursor] {
                    for name in &reference.path {
                        insert_explicit(source, name.range, name.text.clone(), globals, output);
                    }
                }
                for field in &page.cursor_fields {
                    insert_explicit(
                        source,
                        field.range,
                        format!("{target}.{}", field.text),
                        globals,
                        output,
                    );
                }
                collect_expression_references(source, &page.after, globals, output);
                collect_expression_references(source, &page.limit, globals, output);
            }
            if let Some(order) = &query.order {
                insert_explicit(
                    source,
                    order.field.range,
                    format!("{target}.{}", order.field.text),
                    globals,
                    output,
                );
            }
            if let Some(pagination) = &query.pagination {
                collect_expression_references(source, &pagination.limit, globals, output);
                collect_expression_references(source, &pagination.offset, globals, output);
            }
            for include in &query.includes {
                insert_explicit(
                    source,
                    include.relationship.range,
                    format!("{target}.{}", include.relationship.text),
                    globals,
                    output,
                );
                collect_expression_references(source, &include.pagination.limit, globals, output);
                collect_expression_references(source, &include.pagination.offset, globals, output);
            }
        }
        Expression::Update(update) => {
            collect_name_expression(source, &update.target, globals, output);
            let target = joined_name(&update.target);
            insert_explicit(
                source,
                update.field.range,
                format!("{target}.{}", update.field.text),
                globals,
                output,
            );
            collect_expression_references(source, &update.value, globals, output);
            collect_initialisers(source, &target, &update.changes, globals, output);
            for change in &update.conditional_changes {
                collect_initialisers(
                    source,
                    &target,
                    std::slice::from_ref(&change.change),
                    globals,
                    output,
                );
                collect_name_expression(source, &change.supplied, globals, output);
            }
        }
        Expression::Delete(delete) => {
            collect_name_expression(source, &delete.target, globals, output);
            let target = joined_name(&delete.target);
            insert_explicit(
                source,
                delete.field.range,
                format!("{target}.{}", delete.field.text),
                globals,
                output,
            );
            collect_expression_references(source, &delete.value, globals, output);
        }
        Expression::Unary(unary) => {
            collect_expression_references(source, &unary.value, globals, output)
        }
        Expression::Binary(binary) => {
            collect_expression_references(source, &binary.left, globals, output);
            collect_expression_references(source, &binary.right, globals, output);
        }
        Expression::Grouped(grouped) => {
            collect_expression_references(source, &grouped.value, globals, output)
        }
        Expression::Attempt(attempt) => {
            collect_expression_references(source, &attempt.value, globals, output)
        }
        Expression::OutcomeMatch(outcome) => {
            collect_expression_references(source, &outcome.subject, globals, output);
            for arm in &outcome.arms {
                if let jadpo_syntax::OutcomeMatchPattern::Failure(failure) = &arm.pattern {
                    insert_explicit(source, failure.range, failure.text.clone(), globals, output);
                }
                match &arm.body {
                    jadpo_syntax::OutcomeMatchArmBody::Value(value) => {
                        collect_expression_references(source, value, globals, output)
                    }
                    jadpo_syntax::OutcomeMatchArmBody::Reject(rejection) => {
                        insert_explicit(
                            source,
                            rejection.failure.range,
                            rejection.failure.text.clone(),
                            globals,
                            output,
                        );
                        for field in &rejection.values {
                            collect_expression_references(source, &field.value, globals, output);
                        }
                    }
                    jadpo_syntax::OutcomeMatchArmBody::Propagate(_) => {}
                }
            }
        }
    }
}

fn collect_initialisers(
    source: &ParsedSyntax,
    target: &str,
    fields: &[FieldInitialiser],
    globals: &BTreeMap<String, LanguageSymbol>,
    output: &mut BTreeMap<(String, usize, usize), String>,
) {
    for field in fields {
        insert_explicit(
            source,
            field.name.range,
            format!("{target}.{}", field.name.text),
            globals,
            output,
        );
        collect_expression_references(source, &field.value, globals, output);
    }
}

fn collect_name_expression(
    source: &ParsedSyntax,
    expression: &NameExpression,
    globals: &BTreeMap<String, LanguageSymbol>,
    output: &mut BTreeMap<(String, usize, usize), String>,
) {
    for index in 0..expression.path.len() {
        let key = expression.path[..=index]
            .iter()
            .map(|name| name.text.as_str())
            .collect::<Vec<_>>()
            .join(".");
        insert_explicit(source, expression.path[index].range, key, globals, output);
    }
}

fn insert_explicit(
    source: &ParsedSyntax,
    range: TextRange,
    key: String,
    globals: &BTreeMap<String, LanguageSymbol>,
    output: &mut BTreeMap<(String, usize, usize), String>,
) {
    if globals.contains_key(&key) {
        output.insert((source.source_name.clone(), range.start, range.end), key);
    }
}

fn joined_name(expression: &NameExpression) -> String {
    expression
        .path
        .iter()
        .map(|name| name.text.as_str())
        .collect::<Vec<_>>()
        .join(".")
}

fn resolve_token(
    source: &ParsedSyntax,
    project: &AnalyzedProject,
    significant: &[(usize, &Token)],
    position: usize,
    globals: &BTreeMap<String, LanguageSymbol>,
    locals: &[LocalDefinition],
) -> Option<String> {
    let token = significant[position].1;
    let text = token.text(source_text(source));
    let (segments, segment_index) = dotted_path(source, significant, position);

    if segment_index == 0 {
        if let Some(local) = nearest_local(locals, &source.source_name, text, token.range.start) {
            return Some(local.symbol.key.clone());
        }
    }

    let prefix = segments[..=segment_index].join(".");
    if globals.contains_key(&prefix) {
        return Some(prefix);
    }
    let full = segments.join(".");
    if globals.contains_key(&full) && segment_index + 1 == segments.len() {
        return Some(full);
    }
    if let Some(expression) = project
        .typing
        .expressions
        .iter()
        .filter(|expression| {
            expression.source == source.source_name
                && expression.range.start <= token.range.start
                && token.range.end <= expression.range.end
        })
        .min_by_key(|expression| expression.range.end - expression.range.start)
    {
        if globals.contains_key(&expression.type_name)
            && expression
                .type_name
                .rsplit('.')
                .next()
                .is_some_and(|name| name == text)
        {
            return Some(expression.type_name.clone());
        }
    }
    globals.get(text).map(|symbol| symbol.key.clone())
}

fn dotted_path(
    source: &ParsedSyntax,
    significant: &[(usize, &Token)],
    position: usize,
) -> (Vec<String>, usize) {
    let mut start = position;
    while start >= 2
        && significant[start - 1].1.kind == TokenKind::Dot
        && is_name_token(significant[start - 2].1, source)
    {
        start -= 2;
    }
    let mut end = position;
    while end + 2 < significant.len()
        && significant[end + 1].1.kind == TokenKind::Dot
        && is_name_token(significant[end + 2].1, source)
    {
        end += 2;
    }
    let segments = (start..=end)
        .step_by(2)
        .map(|index| significant[index].1.text(source_text(source)).to_owned())
        .collect::<Vec<_>>();
    (segments, (position - start) / 2)
}

fn nearest_local<'definitions>(
    definitions: &'definitions [LocalDefinition],
    source: &str,
    name: &str,
    offset: usize,
) -> Option<&'definitions LocalDefinition> {
    definitions
        .iter()
        .filter(|definition| {
            definition.symbol.source == source
                && definition.symbol.name == name
                && definition.scope.start <= offset
                && offset <= definition.scope.end
                && definition.available_from <= offset
        })
        .max_by_key(|definition| definition.symbol.range.start)
}

fn is_name_token(token: &Token, source: &ParsedSyntax) -> bool {
    let text = token.text(source_text(source));
    text.chars()
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        && !matches!(
            token.kind,
            TokenKind::BooleanLiteral | TokenKind::NoneLiteral | TokenKind::LineComment
        )
}

fn source_text(source: &ParsedSyntax) -> &str {
    &source.source_text
}

#[cfg(test)]
mod tests {
    use super::LanguageIndex;
    use crate::analyze_project;
    use std::path::Path;

    fn repository_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("compiler crate should be inside the repository")
    }

    #[test]
    fn indexes_global_and_local_symbols_with_semantic_references() {
        let fixture = repository_root().join("tests/compile/pass/56_tagged_sum_match.jadpo");
        let analyzed = analyze_project(&fixture).expect("tagged fixture should analyze");
        let index = LanguageIndex::build(&analyzed);

        assert!(index.symbol("PaymentOutcome.paid").is_some());
        assert!(index
            .symbols
            .iter()
            .any(|symbol| symbol.kind == "variable" && symbol.name == "receipt_id"));
        assert!(index.occurrences_for("PaymentOutcome.paid").count() >= 3);
        assert!(
            index
                .occurrences_for("PaymentOutcome.paid.receipt_id")
                .count()
                >= 2
        );
        assert!(index.occurrences_for("OutcomeView.label").count() >= 4);

        let source = &analyzed.syntax.sources[0];
        let needle = "PaymentOutcome.paid {";
        let offset = source
            .source_text
            .find(needle)
            .expect("constructor should exist")
            + "PaymentOutcome.".len();
        assert_eq!(
            index
                .symbol_at(&source.source_name, offset)
                .map(|symbol| symbol.key.as_str()),
            Some("PaymentOutcome.paid")
        );
    }

    #[test]
    fn exposes_inferred_types_at_child_properties() {
        let fixture = repository_root().join("tests/compile/pass/54_scalar_match.jadpo");
        let analyzed = analyze_project(&fixture).expect("scalar match fixture should analyze");
        let index = LanguageIndex::build(&analyzed);
        let source = &analyzed.syntax.sources[0];
        let offset = source
            .source_text
            .find("input.nickname")
            .expect("child selection should exist")
            + "input.".len();

        assert_eq!(
            index.inferred_type_at(&analyzed, &source.source_name, offset),
            Some("MatchInput.nickname?")
        );
    }
}
