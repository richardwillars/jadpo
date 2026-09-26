use crate::SemanticGraph;
use jadpo_diagnostics::{Diagnostic, DiagnosticFact, SourceSpan, TextEdit};
use jadpo_syntax::{
    Block, CallableKind, Declaration, Expression, FieldDeclaration, FieldInitialiser, HttpMethod,
    InvocationExpression, ParsedSyntax, RejectStatement, Statement, TextRange,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FailureContract {
    pub name: String,
    pub kind: String,
    pub http_status: u16,
    pub code: String,
    pub message: Option<String>,
    pub public_fields: Vec<String>,
    pub internal_fields: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableFailureSet {
    pub callable: String,
    pub failures: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteFailure {
    pub route: String,
    pub failure: String,
    pub kind: String,
    pub http_status: u16,
    pub derived: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FailureCheckResult {
    pub contracts: Vec<FailureContract>,
    pub callables: Vec<CallableFailureSet>,
    pub routes: Vec<RouteFailure>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug)]
struct FailureShape {
    contract: FailureContract,
    public_fields: BTreeMap<String, FieldDeclaration>,
    internal_fields: BTreeMap<String, FieldDeclaration>,
}

#[derive(Clone, Debug)]
struct CallSite {
    callee: String,
    range: TextRange,
    attempted: bool,
}

#[derive(Clone, Debug)]
struct RejectSite {
    statement: RejectStatement,
    binding: FailureBinding,
}

#[derive(Clone, Debug)]
struct PersistenceSite {
    range: TextRange,
    acknowledgement_range: TextRange,
    attempted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FailureBinding {
    Direct,
    RequiredQueryMissing,
    RequiredMutationMissing,
    MutationConflict,
    PatchEmpty,
}

#[derive(Clone, Debug)]
struct CallableFacts {
    kind: CallableKind,
    declared: BTreeSet<String>,
    calls: Vec<CallSite>,
    rejects: Vec<RejectSite>,
    persistence: Vec<PersistenceSite>,
    failures_range: Option<TextRange>,
    declaration_range: TextRange,
    source: String,
}

pub fn check_failures(files: &[ParsedSyntax], _graph: &SemanticGraph) -> FailureCheckResult {
    let mut checker = FailureChecker {
        failures: BTreeMap::new(),
        callables: BTreeMap::new(),
        routes: Vec::new(),
        result: FailureCheckResult::default(),
    };
    checker.collect(files);
    checker.validate();
    checker.result
}

struct FailureChecker {
    failures: BTreeMap<String, FailureShape>,
    callables: BTreeMap<String, CallableFacts>,
    routes: Vec<(String, String)>,
    result: FailureCheckResult,
}

impl FailureChecker {
    fn collect(&mut self, files: &[ParsedSyntax]) {
        let mut codes = BTreeMap::<String, (String, String, TextRange)>::new();
        for file in files {
            for declaration in &file.file.declarations {
                match declaration {
                    Declaration::Failure(declaration) => {
                        let public_names = declaration
                            .public_fields
                            .iter()
                            .map(|field| field.name.text.as_str())
                            .collect::<BTreeSet<_>>();
                        for field in &declaration.internal_fields {
                            if public_names.contains(field.name.text.as_str()) {
                                self.push_diagnostic(
                                    Diagnostic::error("FAIL_CONTEXT_FIELD_OVERLAP")
                                        .with_fact(DiagnosticFact::Failure(
                                            declaration.name.text.clone(),
                                        ))
                                        .with_fact(DiagnosticFact::Field(field.name.text.clone())),
                                    &file.source_name,
                                    field.name.range,
                                );
                            }
                        }
                        let code = declaration
                            .code
                            .as_ref()
                            .map(|literal| unquote(&literal.text))
                            .unwrap_or_default();
                        if let Some(code_literal) = &declaration.code {
                            if let Some((first_name, first_source, first_range)) = codes.get(&code)
                            {
                                let diagnostic = Diagnostic::error("FAIL_DUPLICATE_CODE")
                                    .with_fact(DiagnosticFact::Failure(
                                        declaration.name.text.clone(),
                                    ))
                                    .with_fact(DiagnosticFact::OtherFailure(first_name.clone()))
                                    .with_fact(DiagnosticFact::CodeValue(code.clone()))
                                    .with_related(SourceSpan {
                                        source: first_source.clone(),
                                        start: first_range.start,
                                        end: first_range.end,
                                    })
                                    .with_note(format!(
                                        "first code is at {first_source}:{}..{}",
                                        first_range.start, first_range.end
                                    ));
                                self.push_diagnostic(
                                    diagnostic,
                                    &file.source_name,
                                    TextRange::new(
                                        declaration.name.range.start,
                                        code_literal.range.end,
                                    ),
                                );
                            } else {
                                codes.insert(
                                    code.clone(),
                                    (
                                        declaration.name.text.clone(),
                                        file.source_name.clone(),
                                        code_literal.range,
                                    ),
                                );
                            }
                        }
                        let contract = FailureContract {
                            name: declaration.name.text.clone(),
                            kind: declaration.kind.text.clone(),
                            http_status: http_status(&declaration.kind.text).unwrap_or(500),
                            code,
                            message: declaration
                                .message
                                .as_ref()
                                .map(|literal| unquote(&literal.text)),
                            public_fields: sorted_field_names(&declaration.public_fields),
                            internal_fields: sorted_field_names(&declaration.internal_fields),
                        };
                        self.failures.insert(
                            contract.name.clone(),
                            FailureShape {
                                contract,
                                public_fields: field_map(&declaration.public_fields),
                                internal_fields: field_map(&declaration.internal_fields),
                            },
                        );
                    }
                    Declaration::Callable(declaration) => {
                        let mut declared = BTreeSet::new();
                        for failure in &declaration.failures {
                            if !declared.insert(failure.text.clone()) {
                                self.push_diagnostic(
                                    Diagnostic::error("FAIL_DUPLICATE_DECLARATION")
                                        .with_fact(DiagnosticFact::Callable(
                                            declaration.name.text.clone(),
                                        ))
                                        .with_fact(DiagnosticFact::Failure(failure.text.clone())),
                                    &file.source_name,
                                    failure.range,
                                );
                            }
                        }
                        let mut calls = Vec::new();
                        let mut rejects = Vec::new();
                        let mut persistence = Vec::new();
                        collect_block(
                            &declaration.body,
                            &mut calls,
                            &mut rejects,
                            &mut persistence,
                        );
                        self.callables.insert(
                            declaration.name.text.clone(),
                            CallableFacts {
                                kind: declaration.kind,
                                declared,
                                calls,
                                rejects,
                                persistence,
                                failures_range: declaration.failures_range,
                                declaration_range: declaration.range,
                                source: file.source_name.clone(),
                            },
                        );
                    }
                    Declaration::Route(declaration) => {
                        if let Some(run) = &declaration.run {
                            self.routes.push((
                                format!("{} {}", method_name(declaration.method), declaration.path),
                                joined_name(&run.callee.path),
                            ));
                        }
                        if let Some(action) = &declaration.inline_action {
                            let route_name = format!(
                                "{} {} inline action",
                                method_name(declaration.method),
                                declaration.path
                            );
                            let mut calls = Vec::new();
                            let mut rejects = Vec::new();
                            let mut persistence = Vec::new();
                            let mut declared = BTreeSet::new();
                            for failure in &action.failures {
                                if !declared.insert(failure.text.clone()) {
                                    self.push_diagnostic(
                                        Diagnostic::error("FAIL_DUPLICATE_DECLARATION")
                                            .with_fact(DiagnosticFact::Callable(route_name.clone()))
                                            .with_fact(DiagnosticFact::Failure(
                                                failure.text.clone(),
                                            )),
                                        &file.source_name,
                                        failure.range,
                                    );
                                }
                            }
                            collect_block(&action.body, &mut calls, &mut rejects, &mut persistence);
                            self.callables.insert(
                                route_name.clone(),
                                CallableFacts {
                                    kind: CallableKind::Action,
                                    declared,
                                    calls,
                                    rejects,
                                    persistence,
                                    failures_range: action.failures_range,
                                    declaration_range: action.range,
                                    source: file.source_name.clone(),
                                },
                            );
                            self.routes.push((
                                format!("{} {}", method_name(declaration.method), declaration.path),
                                route_name,
                            ));
                        }
                    }
                    Declaration::Type(_)
                    | Declaration::Enum(_)
                    | Declaration::Record(_)
                    | Declaration::Test(_) => {}
                }
            }
        }
    }

    fn validate(&mut self) {
        for shape in self.failures.values() {
            self.result.contracts.push(shape.contract.clone());
        }

        for (name, facts) in self.callables.clone() {
            let mut reachable = BTreeSet::new();
            if facts.kind == CallableKind::Function {
                for site in &facts.persistence {
                    self.push_diagnostic(
                        Diagnostic::error("EFFECT_FUNCTION_PERSISTENCE")
                            .with_fact(DiagnosticFact::Callable(name.clone())),
                        &facts.source,
                        site.range,
                    );
                }
            }
            for site in &facts.persistence {
                if !site.attempted {
                    self.push_diagnostic(
                        Diagnostic::error("FAIL_ATTEMPT_REQUIRED")
                            .with_fact(DiagnosticFact::Usage("persistence operation".to_owned()))
                            .with_edit(TextEdit {
                                source: facts.source.clone(),
                                start: site.range.start,
                                end: site.range.start,
                                replacement: "attempt ".to_owned(),
                            }),
                        &facts.source,
                        site.acknowledgement_range,
                    );
                }
            }
            for reject in &facts.rejects {
                let failure = &reject.statement.failure.text;
                reachable.insert(failure.clone());
                if !facts.declared.contains(failure) {
                    self.push_diagnostic(
                        Diagnostic::error("FAIL_UNDECLARED_PROPAGATION")
                            .with_fact(DiagnosticFact::Callable(name.clone()))
                            .with_fact(DiagnosticFact::Failure(failure.clone())),
                        &facts.source,
                        TextRange::new(
                            reject.statement.range.start,
                            reject.statement.failure.range.end,
                        ),
                    );
                }
                self.validate_reject_payload(&reject.statement, &facts.source);
                if let Some(shape) = self.failures.get(failure) {
                    let requirement = match reject.binding {
                        FailureBinding::Direct => None,
                        FailureBinding::RequiredQueryMissing => Some((
                            "NotFound",
                            "FAIL_REQUIRED_QUERY_NOT_NOT_FOUND",
                            "required query missing",
                        )),
                        FailureBinding::RequiredMutationMissing => Some((
                            "NotFound",
                            "FAIL_REQUIRED_MUTATION_NOT_NOT_FOUND",
                            "required mutation missing",
                        )),
                        FailureBinding::MutationConflict => Some((
                            "Conflict",
                            "FAIL_MUTATION_CONFLICT_NOT_CONFLICT",
                            "mutation conflict",
                        )),
                        FailureBinding::PatchEmpty => Some((
                            "InvalidValue",
                            "FAIL_PATCH_EMPTY_NOT_INVALID_VALUE",
                            "empty patch",
                        )),
                    };
                    if let Some((kind, code, _label)) = requirement {
                        if shape.contract.kind != kind {
                            let usage = match reject.binding {
                                FailureBinding::RequiredQueryMissing
                                | FailureBinding::RequiredMutationMissing => "`missing:`",
                                FailureBinding::MutationConflict => "`conflict:`",
                                FailureBinding::PatchEmpty => "`empty:`",
                                FailureBinding::Direct => "this binding",
                            };
                            self.push_diagnostic(
                                Diagnostic::error(code)
                                    .with_fact(DiagnosticFact::Failure(failure.clone()))
                                    .with_fact(DiagnosticFact::Expected(kind.to_owned()))
                                    .with_fact(DiagnosticFact::Received(
                                        shape.contract.kind.clone(),
                                    ))
                                    .with_fact(DiagnosticFact::Usage(usage.to_owned())),
                                &facts.source,
                                reject.statement.failure.range,
                            );
                        }
                    }
                }
            }

            for call in &facts.calls {
                let Some(callee) = self.callables.get(&call.callee).cloned() else {
                    continue;
                };
                if facts.kind == CallableKind::Function && callee.kind == CallableKind::Action {
                    self.push_diagnostic(
                        Diagnostic::error("EFFECT_FUNCTION_CALLS_ACTION")
                            .with_fact(DiagnosticFact::Callable(name.clone()))
                            .with_fact(DiagnosticFact::Name(call.callee.clone())),
                        &facts.source,
                        call.range,
                    );
                }
                if !callee.declared.is_empty() && !call.attempted {
                    self.push_diagnostic(
                        Diagnostic::error("FAIL_ATTEMPT_REQUIRED")
                            .with_fact(DiagnosticFact::Usage("call".to_owned()))
                            .with_fact(DiagnosticFact::Operation(call.callee.clone()))
                            .with_edit(TextEdit {
                                source: facts.source.clone(),
                                start: call.range.start,
                                end: call.range.start,
                                replacement: "attempt ".to_owned(),
                            }),
                        &facts.source,
                        call.range,
                    );
                }
                for failure in &callee.declared {
                    reachable.insert(failure.clone());
                    if !facts.declared.contains(failure) {
                        self.push_diagnostic(
                            Diagnostic::error("FAIL_UNDECLARED_PROPAGATION")
                                .with_fact(DiagnosticFact::Callable(name.clone()))
                                .with_fact(DiagnosticFact::Failure(failure.clone())),
                            &facts.source,
                            call.range,
                        );
                    }
                }
            }

            let stale = facts
                .declared
                .difference(&reachable)
                .cloned()
                .collect::<Vec<_>>();
            if !stale.is_empty() {
                let range = facts.failures_range.unwrap_or(facts.declaration_range);
                let replacement = if reachable.is_empty() {
                    String::new()
                } else {
                    format!(
                        "fails {}",
                        reachable.iter().cloned().collect::<Vec<_>>().join(", ")
                    )
                };
                let diagnostic = Diagnostic::error("FAIL_STALE_DECLARATION")
                    .with_edit(TextEdit {
                        source: facts.source.clone(),
                        start: range.start,
                        end: range.end,
                        replacement,
                    })
                    .with_fact(DiagnosticFact::Callable(name.clone()))
                    .with_fact(DiagnosticFact::DeclaredProblems(
                        facts.declared.iter().cloned().collect::<Vec<_>>().join(","),
                    ))
                    .with_fact(DiagnosticFact::ReachableProblems(
                        reachable.iter().cloned().collect::<Vec<_>>().join(","),
                    ))
                    .with_fact(DiagnosticFact::Failures(stale.join(", ")));
                self.push_diagnostic(diagnostic, &facts.source, range);
            }

            self.result.callables.push(CallableFailureSet {
                callable: name,
                failures: facts.declared.into_iter().collect(),
            });
        }

        for (route, callee) in &self.routes {
            let Some(facts) = self.callables.get(callee) else {
                continue;
            };
            for failure in &facts.declared {
                if let Some(shape) = self.failures.get(failure) {
                    self.result.routes.push(RouteFailure {
                        route: route.clone(),
                        failure: failure.clone(),
                        kind: shape.contract.kind.clone(),
                        http_status: shape.contract.http_status,
                        derived: true,
                    });
                }
            }
        }

        self.result
            .contracts
            .sort_by(|left, right| left.name.cmp(&right.name));
        self.result
            .callables
            .sort_by(|left, right| left.callable.cmp(&right.callable));
        self.result.routes.sort_by(|left, right| {
            (&left.route, &left.failure).cmp(&(&right.route, &right.failure))
        });
    }

    fn validate_reject_payload(&mut self, reject: &RejectStatement, source: &str) {
        let Some(shape) = self.failures.get(&reject.failure.text).cloned() else {
            return;
        };
        let mut expected = shape.public_fields;
        let mut scopes = expected
            .keys()
            .map(|field| (field.clone(), "public".to_owned()))
            .collect::<BTreeMap<_, _>>();
        scopes.extend(
            shape
                .internal_fields
                .keys()
                .map(|field| (field.clone(), "internal".to_owned())),
        );
        expected.extend(shape.internal_fields);
        self.validate_context(
            &shape.contract.name,
            &expected,
            &scopes,
            &reject.values,
            reject.range,
            source,
        );
    }

    fn validate_context(
        &mut self,
        failure: &str,
        expected: &BTreeMap<String, FieldDeclaration>,
        scopes: &BTreeMap<String, String>,
        supplied: &[FieldInitialiser],
        rejection_range: TextRange,
        source: &str,
    ) {
        let mut seen = BTreeSet::new();
        for field in supplied {
            if !seen.insert(field.name.text.clone()) {
                self.push_diagnostic(
                    Diagnostic::error("FAIL_DUPLICATE_CONTEXT_FIELD")
                        .with_fact(DiagnosticFact::Failure(failure.to_owned()))
                        .with_fact(DiagnosticFact::Field(field.name.text.clone())),
                    source,
                    field.name.range,
                );
            }
            if !expected.contains_key(&field.name.text) {
                self.push_diagnostic(
                    Diagnostic::error("FAIL_UNKNOWN_CONTEXT_FIELD")
                        .with_fact(DiagnosticFact::Failure(failure.to_owned()))
                        .with_fact(DiagnosticFact::Field(field.name.text.clone())),
                    source,
                    field.name.range,
                );
            }
        }
        for (field, declaration) in expected {
            if !declaration.optional && !seen.contains(field) {
                self.push_diagnostic(
                    Diagnostic::error("FAIL_MISSING_CONTEXT_FIELD")
                        .with_fact(DiagnosticFact::Failure(failure.to_owned()))
                        .with_fact(DiagnosticFact::Field(field.clone()))
                        .with_fact(DiagnosticFact::Scope(
                            scopes
                                .get(field)
                                .cloned()
                                .unwrap_or_else(|| "required".to_owned()),
                        )),
                    source,
                    rejection_range,
                );
            }
        }
    }

    fn push_diagnostic(&mut self, mut diagnostic: Diagnostic, source: &str, range: TextRange) {
        diagnostic.primary = Some(SourceSpan {
            source: source.to_owned(),
            start: range.start,
            end: range.end,
        });
        self.result.diagnostics.push(diagnostic);
    }
}

fn collect_block(
    block: &Block,
    calls: &mut Vec<CallSite>,
    rejects: &mut Vec<RejectSite>,
    persistence: &mut Vec<PersistenceSite>,
) {
    for statement in &block.statements {
        match statement {
            Statement::Binding(statement) => {
                collect_expression(&statement.value, calls, rejects, persistence)
            }
            Statement::Assignment(statement) => {
                collect_expression(&statement.value, calls, rejects, persistence)
            }
            Statement::Return(statement) => {
                collect_expression(&statement.value, calls, rejects, persistence)
            }
            Statement::Reject(statement) => {
                rejects.push(RejectSite {
                    statement: statement.clone(),
                    binding: FailureBinding::Direct,
                });
                for field in &statement.values {
                    collect_expression(&field.value, calls, rejects, persistence);
                }
            }
            Statement::If(statement) => {
                collect_expression(&statement.condition, calls, rejects, persistence);
                collect_block(&statement.then_block, calls, rejects, persistence);
                if let Some(else_block) = &statement.else_block {
                    collect_block(else_block, calls, rejects, persistence);
                }
            }
            Statement::Match(statement) => {
                collect_expression(&statement.subject, calls, rejects, persistence);
                for arm in &statement.arms {
                    collect_block(&arm.body, calls, rejects, persistence);
                }
            }
            Statement::Assert(statement) => {
                collect_expression(&statement.condition, calls, rejects, persistence)
            }
            Statement::Unsupported(_) => {}
        }
    }
}

fn collect_expression(
    expression: &Expression,
    calls: &mut Vec<CallSite>,
    rejects: &mut Vec<RejectSite>,
    persistence: &mut Vec<PersistenceSite>,
) {
    collect_expression_inner(expression, calls, rejects, persistence, false);
}

fn collect_expression_inner(
    expression: &Expression,
    calls: &mut Vec<CallSite>,
    rejects: &mut Vec<RejectSite>,
    persistence: &mut Vec<PersistenceSite>,
    attempted: bool,
) {
    match expression {
        Expression::Invocation(invocation) => {
            calls.push(call_site(invocation, attempted));
            for argument in &invocation.arguments {
                collect_expression(argument, calls, rejects, persistence);
            }
        }
        Expression::Construction(construction) => {
            for field in &construction.fields {
                collect_expression(&field.value, calls, rejects, persistence);
            }
        }
        Expression::Create(create) => {
            persistence.push(PersistenceSite {
                range: create.range,
                acknowledgement_range: TextRange::new(create.range.start, create.range.start + 6),
                attempted,
            });
            for field in &create.fields {
                collect_expression(&field.value, calls, rejects, persistence);
            }
            for conflict in &create.conflicts {
                collect_failure_binding(
                    &conflict.rejection,
                    FailureBinding::MutationConflict,
                    calls,
                    rejects,
                    persistence,
                );
            }
        }
        Expression::Query(query) => {
            persistence.push(PersistenceSite {
                range: query.range,
                acknowledgement_range: TextRange::new(query.range.start, query.range.start + 5),
                attempted,
            });
            collect_expression(&query.value, calls, rejects, persistence);
            if let Some(pagination) = &query.pagination {
                collect_expression(&pagination.limit, calls, rejects, persistence);
                collect_expression(&pagination.offset, calls, rejects, persistence);
            }
            for include in &query.includes {
                collect_expression(&include.pagination.limit, calls, rejects, persistence);
                collect_expression(&include.pagination.offset, calls, rejects, persistence);
            }
            if let Some(missing) = &query.missing {
                rejects.push(RejectSite {
                    statement: missing.clone(),
                    binding: FailureBinding::RequiredQueryMissing,
                });
                for field in &missing.values {
                    collect_expression(&field.value, calls, rejects, persistence);
                }
            }
        }
        Expression::Update(update) => {
            persistence.push(PersistenceSite {
                range: update.range,
                acknowledgement_range: TextRange::new(update.range.start, update.range.start + 6),
                attempted,
            });
            collect_expression(&update.value, calls, rejects, persistence);
            for change in &update.changes {
                collect_expression(&change.value, calls, rejects, persistence);
            }
            for conditional in &update.conditional_changes {
                collect_expression(&conditional.change.value, calls, rejects, persistence);
            }
            if let Some(empty) = &update.empty {
                collect_failure_binding(
                    empty,
                    FailureBinding::PatchEmpty,
                    calls,
                    rejects,
                    persistence,
                );
            }
            collect_failure_binding(
                &update.missing,
                FailureBinding::RequiredMutationMissing,
                calls,
                rejects,
                persistence,
            );
            for conflict in &update.conflicts {
                collect_failure_binding(
                    &conflict.rejection,
                    FailureBinding::MutationConflict,
                    calls,
                    rejects,
                    persistence,
                );
            }
        }
        Expression::Delete(delete) => {
            persistence.push(PersistenceSite {
                range: delete.range,
                acknowledgement_range: TextRange::new(delete.range.start, delete.range.start + 6),
                attempted,
            });
            collect_expression(&delete.value, calls, rejects, persistence);
            collect_failure_binding(
                &delete.missing,
                FailureBinding::RequiredMutationMissing,
                calls,
                rejects,
                persistence,
            );
            for conflict in &delete.conflicts {
                collect_failure_binding(
                    &conflict.rejection,
                    FailureBinding::MutationConflict,
                    calls,
                    rejects,
                    persistence,
                );
            }
        }
        Expression::Binary(binary) => {
            collect_expression(&binary.left, calls, rejects, persistence);
            collect_expression(&binary.right, calls, rejects, persistence);
        }
        Expression::Unary(unary) => collect_expression(&unary.value, calls, rejects, persistence),
        Expression::Grouped(grouped) => {
            collect_expression(&grouped.value, calls, rejects, persistence)
        }
        Expression::Attempt(attempt) => {
            collect_expression_inner(&attempt.value, calls, rejects, persistence, true)
        }
        Expression::Literal(_) | Expression::Name(_) | Expression::Missing(_) => {}
    }
}

fn collect_failure_binding(
    binding: &RejectStatement,
    kind: FailureBinding,
    calls: &mut Vec<CallSite>,
    rejects: &mut Vec<RejectSite>,
    persistence: &mut Vec<PersistenceSite>,
) {
    rejects.push(RejectSite {
        statement: binding.clone(),
        binding: kind,
    });
    for field in &binding.values {
        collect_expression(&field.value, calls, rejects, persistence);
    }
}

fn call_site(invocation: &InvocationExpression, attempted: bool) -> CallSite {
    CallSite {
        callee: joined_name(&invocation.callee.path),
        range: invocation.range,
        attempted,
    }
}

fn field_map(fields: &[FieldDeclaration]) -> BTreeMap<String, FieldDeclaration> {
    fields
        .iter()
        .map(|field| (field.name.text.clone(), field.clone()))
        .collect()
}

fn sorted_field_names(fields: &[FieldDeclaration]) -> Vec<String> {
    let mut names = fields
        .iter()
        .map(|field| field.name.text.clone())
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn joined_name(path: &[jadpo_syntax::Name]) -> String {
    path.iter()
        .map(|name| name.text.as_str())
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

fn http_status(kind: &str) -> Option<u16> {
    match kind {
        "InvalidRequest" => Some(400),
        "Unauthenticated" => Some(401),
        "NotPermitted" => Some(403),
        "NotVisible" | "NotFound" => Some(404),
        "Conflict" => Some(409),
        "PreconditionFailed" => Some(412),
        "InvalidValue" | "Rejected" => Some(422),
        "RateLimited" => Some(429),
        "Unavailable" => Some(503),
        "TimedOut" => Some(504),
        "Misconfigured" | "InternalFault" => Some(500),
        _ => None,
    }
}

fn unquote(value: &str) -> String {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::check_failures;
    use crate::build_semantic_graph;
    use jadpo_syntax::parse;
    use std::path::Path;

    fn check(source: &str) -> super::FailureCheckResult {
        let parsed = parse(Path::new("test.jadpo"), source);
        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let graph = build_semantic_graph(std::slice::from_ref(&parsed));
        assert!(graph.diagnostics.is_empty(), "{:#?}", graph.diagnostics);
        check_failures(&[parsed], &graph)
    }

    fn apply_preferred_edit(source: &str, diagnostic: &jadpo_diagnostics::Diagnostic) -> String {
        let edit = diagnostic
            .recommended_next_step
            .edits
            .first()
            .expect("automatic repair should contain an edit");
        format!(
            "{}{}{}",
            &source[..edit.start],
            edit.replacement,
            &source[edit.end..]
        )
    }

    #[test]
    fn derives_route_status_from_failure_kind() {
        let result = check(
            r#"
value Result { ok: Bool }
failure Missing { kind: NotFound code: "missing" }
action find() -> Result fails Missing { reject Missing }
route GET /result { auth: none output: Result run: find() }
"#,
        );
        assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
        assert_eq!(result.routes.len(), 1);
        assert_eq!(result.routes[0].http_status, 404);
        assert!(result.routes[0].derived);
    }

    #[test]
    fn rejects_undeclared_direct_failures() {
        let result = check(
            r#"
value Result { ok: Bool }
failure Closed { kind: Conflict code: "closed" }
action register() -> Result { reject Closed }
"#,
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "FAIL_UNDECLARED_PROPAGATION");
    }

    #[test]
    fn requires_create_conflicts_to_use_the_conflict_failure_kind() {
        let result = check(
            r#"
entity Account { id: Uuid identity }
failure Refused { kind: Rejected code: "refused" }
action add(id: Account.id) -> Account fails Refused {
    return attempt create Account { id: id } conflict: Refused
}
"#,
        );

        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "FAIL_MUTATION_CONFLICT_NOT_CONFLICT"));
    }

    #[test]
    fn preferred_attempt_repair_removes_only_the_named_diagnostic() {
        let source = r#"
value Result { ok: Bool }
failure Refused { kind: Rejected code: "refused" }
action child() -> Result fails Refused { reject Refused }
action parent() -> Result fails Refused { return child() }
"#;
        let before = check(source);
        let diagnostic = before
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "FAIL_ATTEMPT_REQUIRED")
            .expect("unacknowledged call should be diagnosed");
        let repaired = apply_preferred_edit(source, diagnostic);
        let after = check(&repaired);

        assert!(after.diagnostics.is_empty(), "{:#?}", after.diagnostics);
        assert_eq!(before.contracts, after.contracts);
    }

    #[test]
    fn preferred_stale_fails_repair_preserves_the_public_failure_contract() {
        let source = r#"
value Result { ok: Bool }
failure Refused { kind: Rejected code: "refused" }
action parent() -> Result fails Refused { return Result { ok: true } }
"#;
        let before = check(source);
        let diagnostic = before
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "FAIL_STALE_DECLARATION")
            .expect("stale failure should be diagnosed");
        let repaired = apply_preferred_edit(source, diagnostic);
        let after = check(&repaired);

        assert!(after.diagnostics.is_empty(), "{:#?}", after.diagnostics);
        assert_eq!(before.contracts, after.contracts);
    }

    #[test]
    fn stale_fails_repair_names_only_the_removed_failure_and_preserves_reachable_ones() {
        let source = r#"
value Result { ok: Bool }
failure Refused { kind: Rejected code: "refused" }
failure Closed { kind: Conflict code: "closed" }
action parent() -> Result fails Refused, Closed { reject Refused }
"#;
        let before = check(source);
        let diagnostic = before
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "FAIL_STALE_DECLARATION")
            .expect("stale failure should be diagnosed");

        assert_eq!(diagnostic.message, "`Closed` can never leave `parent`");
        assert_eq!(
            diagnostic.recommended_next_step.title,
            "Remove `Closed` from `parent`'s `fails` list"
        );
        assert_eq!(
            diagnostic.recommended_next_step.edits[0].replacement,
            "fails Refused"
        );

        let repaired = apply_preferred_edit(source, diagnostic);
        assert!(repaired.contains("action parent() -> Result fails Refused {"));
        assert!(check(&repaired).diagnostics.is_empty());
    }
}
