//! Necessary structural facts only. No checked binding, graph node or execution
//! authority is published here; the complete core finish boundary owns those.
use jadpo_diagnostics::{Diagnostic, SourceSpan};
use jadpo_semantic::{NodeKind, SemanticGraph};
use jadpo_syntax::*;
use std::collections::{BTreeMap, BTreeSet};
mod facts;
pub use facts::DeliveryHookCandidate;
use facts::{HookKind, HookSiteCandidate};

pub(crate) fn check_delivery_hooks(
    files: &[ParsedSyntax],
    graph: &SemanticGraph,
) -> (Vec<Diagnostic>, Vec<DeliveryHookCandidate>) {
    let declarations = files
        .iter()
        .flat_map(|file| &file.file.declarations)
        .collect::<Vec<_>>();
    let records = declarations
        .iter()
        .filter_map(|declaration| match declaration {
            Declaration::Record(record) => Some((record.name.text.as_str(), record)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let aliases = declarations
        .iter()
        .filter_map(|declaration| match declaration {
            Declaration::Type(ty) => Some((ty.name.text.as_str(), &ty.parent)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let callables = files
        .iter()
        .flat_map(|file| {
            file.file
                .declarations
                .iter()
                .filter_map(move |declaration| {
                    let Declaration::Callable(callable) = declaration else {
                        return None;
                    };
                    Some((qualified(callable), (callable, file.source_name.as_str())))
                })
        })
        .collect::<BTreeMap<_, _>>();
    let mut diagnostics = Vec::new();
    let mut candidates = Vec::new();
    for file in files {
        for declaration in &file.file.declarations {
            let Declaration::Job(job) = declaration else {
                continue;
            };
            let Some(delivery) = &job.delivery else {
                continue;
            };
            let start_errors = diagnostics.len();
            let mut resolved_sites = Vec::new();
            let mut census = Census {
                graph,
                records: &records,
                aliases: &aliases,
                delivery,
                diagnostics: &mut diagnostics,
                allowed: BTreeSet::new(),
            };
            if [
                &delivery.selection.due,
                &delivery.selection.unsent,
                &delivery.selection.open_field,
                &delivery.selection.open_variant,
            ]
            .iter()
            .any(|reference| reference.path.len() != 2)
            {
                census.reject(&file.source_name, delivery.selection.range);
                continue;
            }
            // Only a direct, compatible site in each exact checked action can
            // enter this allowlist. Naming a callable never exempts its body.
            for (hook, is_create) in [
                (&delivery.hooks.create, true),
                (&delivery.hooks.patch, false),
            ] {
                let name = joined(&hook.path);
                let Some((callable, source)) = callables.get(&name) else {
                    census.reject(&file.source_name, hook.range);
                    continue;
                };
                if graph.node(&name).map(|node| node.kind) != Some(NodeKind::Action)
                    || callable.kind != CallableKind::Action
                    || callable.owner.as_ref().map(|owner| &owner.text)
                        != Some(&delivery.selection.entity.text)
                {
                    census.reject(source, hook.range);
                    continue;
                }
                let sites = callable
                    .body
                    .statements
                    .iter()
                    .filter_map(|statement| {
                        let Statement::Binding(binding) = statement else {
                            return None;
                        };
                        let Expression::Attempt(attempt) = &binding.value else {
                            return None;
                        };
                        match attempt.value.as_ref() {
                            Expression::Create(create)
                                if is_create && census.create_matches(create, callable) =>
                            {
                                census
                                    .create_site(create, callable)
                                    .map(|fact| (create.range, fact))
                            }
                            Expression::Update(update)
                                if !is_create && census.patch_matches(update, callable) =>
                            {
                                census
                                    .patch_site(update, callable)
                                    .map(|fact| (update.range, fact))
                            }
                            _ => None,
                        }
                    })
                    .collect::<Vec<_>>();
                if let [(range, fact)] = sites.as_slice() {
                    census
                        .allowed
                        .insert((source.to_string(), range.start, range.end));
                    resolved_sites.push(fact.clone());
                } else {
                    census.reject(source, callable.range);
                }
            }
            // The run action is an ordinary effect-free shell, not activation.
            if job.run.as_ref().map_or(true, |run| {
                callables
                    .get(&joined(&run.callee.path))
                    .map_or(true, |(callable, _)| !callable.body.statements.is_empty())
            }) {
                census.reject(&file.source_name, job.range);
            }
            for source in files {
                for declaration in &source.file.declarations {
                    match declaration {
                        Declaration::Callable(callable) => {
                            census.block(&callable.body, &callable.parameters, &source.source_name)
                        }
                        Declaration::Route(route) => {
                            if let Some(run) = &route.run {
                                census.expression(
                                    &Expression::Invocation(run.clone()),
                                    &[],
                                    &source.source_name,
                                );
                            }
                            if let Some(action) = &route.inline_action {
                                census.block(&action.body, &[], &source.source_name);
                            }
                        }
                        Declaration::Test(test) => {
                            census.block(&test.body, &[], &source.source_name)
                        }
                        Declaration::Record(record) => {
                            if record.name.text == delivery.selection.entity.text {
                                for field in &record.fields {
                                    if census.bound_field(&field.name.text)
                                        && field.generated.is_some()
                                    {
                                        census.reject(&source.source_name, field.range);
                                    }
                                }
                            }
                            if let Some(lifecycle) = record
                                .dossier
                                .as_ref()
                                .and_then(|dossier| dossier.lifecycle.as_ref())
                            {
                                for field in lifecycle.initial.iter().flatten().chain(
                                    lifecycle
                                        .transitions
                                        .iter()
                                        .flat_map(|transition| &transition.set),
                                ) {
                                    if record.name.text == delivery.selection.entity.text
                                        && census.bound_field(&field.name.text)
                                    {
                                        census.reject(&source.source_name, field.range);
                                    }
                                    census.expression(&field.value, &[], &source.source_name);
                                }
                                if let Some(visible) = &lifecycle.visible {
                                    census.expression(visible, &[], &source.source_name);
                                }
                                for transition in &lifecycle.transitions {
                                    census.expression(&transition.from, &[], &source.source_name);
                                }
                                if let Some(purge) = &lifecycle.purge {
                                    census.expression(&purge.after, &[], &source.source_name);
                                }
                            }
                        }
                        Declaration::Fixture(fixture) => {
                            if let Some(clock) = &fixture.clock {
                                census.expression(clock, &[], &source.source_name);
                            }
                            for field in fixture.configuration.iter().flatten() {
                                census.expression(&field.value, &[], &source.source_name);
                            }
                            for outcome in
                                fixture.service_fakes.iter().flat_map(|fake| &fake.outcomes)
                            {
                                if let FixtureServiceFakeValue::Accepted(value) = &outcome.value {
                                    census.expression(value, &[], &source.source_name);
                                }
                            }
                        }
                        Declaration::Job(job) => {
                            if let Some(run) = &job.run {
                                census.expression(
                                    &Expression::Invocation(run.clone()),
                                    &[],
                                    &source.source_name,
                                );
                            }
                        }
                        Declaration::AuthenticationStrategy(strategy) => {
                            for validator in &strategy.validators {
                                for setting in &validator.settings {
                                    census.expression(&setting.value, &[], &source.source_name);
                                }
                                if let Some(credentials) = &validator.credentials {
                                    census.expression(
                                        &credentials.active,
                                        &[],
                                        &source.source_name,
                                    );
                                }
                            }
                            for resolution in &strategy.resolutions {
                                census.expression(&resolution.active, &[], &source.source_name);
                            }
                        }
                        _ => {}
                    }
                }
            }
            if census.diagnostics.len() == start_errors {
                if let [create, patch] = resolved_sites.as_slice() {
                    candidates.push(DeliveryHookCandidate::new(
                        job.name.text.clone(),
                        file.source_name.clone(),
                        create.clone(),
                        patch.clone(),
                    ));
                }
            }
        }
    }
    if !diagnostics.is_empty() {
        candidates.clear();
    }
    (diagnostics, candidates)
}

fn joined(path: &[Name]) -> String {
    path.iter()
        .map(|name| name.text.as_str())
        .collect::<Vec<_>>()
        .join(".")
}
fn qualified(callable: &CallableDeclaration) -> String {
    // Parser-owned callables already carry their fully qualified identity.
    callable.name.text.clone()
}
fn is_path(expression: &Expression, expected: &[&str]) -> bool {
    matches!(expression, Expression::Name(name) if name.path.iter().map(|part| part.text.as_str()).eq(expected.iter().copied()))
}
fn is_none(expression: &Expression) -> bool {
    matches!(expression, Expression::Literal(literal) if literal.kind == LiteralKind::None)
}

struct Census<'a, 'b> {
    graph: &'a SemanticGraph,
    records: &'a BTreeMap<&'a str, &'a RecordDeclaration>,
    aliases: &'a BTreeMap<&'a str, &'a TypeReference>,
    delivery: &'a JobReminderDelivery,
    allowed: BTreeSet<(String, usize, usize)>,
    diagnostics: &'b mut Vec<Diagnostic>,
}
impl Census<'_, '_> {
    fn site(
        &self,
        kind: HookKind,
        callable: &CallableDeclaration,
        supplied: &NameExpression,
    ) -> Option<HookSiteCandidate> {
        let (parameter, field) = self.parameter_field(callable, supplied)?;
        let record = self.record(&parameter.parameter_type)?;
        let parameter_index = callable
            .parameters
            .iter()
            .position(|candidate| candidate.name == parameter.name)?;
        Some(HookSiteCandidate::new(
            kind,
            qualified(callable),
            parameter_index,
            parameter.name.text.clone(),
            joined(&parameter.parameter_type.path),
            format!("{}.{}", record.name.text, field.name.text),
            joined(&self.delivery.selection.due.path),
            joined(&self.delivery.selection.unsent.path),
            (kind == HookKind::Create).then(|| {
                (
                    joined(&self.delivery.selection.open_field.path),
                    joined(&self.delivery.selection.open_variant.path),
                )
            }),
        ))
    }
    fn create_site(
        &self,
        create: &CreateExpression,
        callable: &CallableDeclaration,
    ) -> Option<HookSiteCandidate> {
        let due_name = &self.delivery.selection.due.path.last()?.text;
        let due = create
            .fields
            .iter()
            .find(|field| &field.name.text == due_name)?;
        let Expression::Name(supplied) = &due.value else {
            return None;
        };
        self.site(HookKind::Create, callable, supplied)
    }
    fn patch_site(
        &self,
        update: &UpdateExpression,
        callable: &CallableDeclaration,
    ) -> Option<HookSiteCandidate> {
        self.site(
            HookKind::Patch,
            callable,
            &update.conditional_changes.first()?.supplied,
        )
    }
    fn reject(&mut self, source: &str, range: TextRange) {
        let mut diagnostic = Diagnostic::error("TYPE_JOB_DELIVERY_HOOK_INVALID");
        diagnostic.primary = Some(SourceSpan {
            source: source.to_owned(),
            start: range.start,
            end: range.end,
        });
        self.diagnostics.push(diagnostic);
    }
    fn bound_field(&self, field: &str) -> bool {
        [
            self.delivery.selection.due.path.last(),
            self.delivery.selection.unsent.path.last(),
        ]
        .into_iter()
        .flatten()
        .any(|name| name.text == field)
    }
    fn exact_entity(&self, target: &NameExpression) -> bool {
        let name = joined(&target.path);
        name == self.delivery.selection.entity.text
            && self.graph.node(&name).map(|node| node.kind) == Some(NodeKind::Entity)
    }
    fn record(&self, reference: &TypeReference) -> Option<&RecordDeclaration> {
        if reference.nullable || !reference.arguments.is_empty() {
            return None;
        }
        let mut name = joined(&reference.path);
        let mut visited = BTreeSet::new();
        while visited.insert(name.clone()) {
            if let Some(record) = self.records.get(name.as_str()) {
                return Some(record);
            }
            let parent = self.aliases.get(name.as_str())?;
            if parent.nullable || !parent.arguments.is_empty() {
                return None;
            }
            name = joined(&parent.path);
        }
        None
    }
    fn due_reference(&self, reference: &TypeReference) -> bool {
        let mut name = joined(&reference.path);
        let mut visited = BTreeSet::new();
        while visited.insert(name.clone()) {
            if name == joined(&self.delivery.selection.due.path) {
                return reference.arguments.is_empty();
            }
            let Some(parent) = self.aliases.get(name.as_str()) else {
                return false;
            };
            if !parent.arguments.is_empty() {
                return false;
            }
            name = joined(&parent.path);
        }
        false
    }
    fn parameter_field<'c>(
        &'c self,
        callable: &'c CallableDeclaration,
        path: &NameExpression,
    ) -> Option<(&Parameter, &FieldDeclaration)> {
        let [parameter_name, field_name] = path.path.as_slice() else {
            return None;
        };
        // Shadowing or reassignment is not parameter-origin evidence. A future
        // alias solver may relax this, but unknown origin must never be empty.
        if parameter_written(&callable.body, &parameter_name.text) {
            return None;
        }
        let parameter = callable
            .parameters
            .iter()
            .find(|parameter| parameter.name.text == parameter_name.text)?;
        let field = self
            .record(&parameter.parameter_type)?
            .fields
            .iter()
            .find(|field| field.name.text == field_name.text)?;
        Some((parameter, field))
    }
    fn create_matches(&self, create: &CreateExpression, callable: &CallableDeclaration) -> bool {
        if !self.exact_entity(&create.target) {
            return false;
        }
        let due_name = &self.delivery.selection.due.path.last().unwrap().text;
        let sent_name = &self.delivery.selection.unsent.path.last().unwrap().text;
        let status_name = &self.delivery.selection.open_field.path.last().unwrap().text;
        let fields = create
            .fields
            .iter()
            .map(|field| (field.name.text.as_str(), &field.value))
            .collect::<BTreeMap<_, _>>();
        if fields.len() != create.fields.len()
            || !fields
                .get(sent_name.as_str())
                .is_some_and(|value| is_none(value))
        {
            return false;
        }
        let Some(Expression::Invocation(status)) = fields.get(status_name.as_str()).copied() else {
            return false;
        };
        if joined(&status.callee.path) != joined(&self.delivery.selection.open_field.path)
            || !status.named_arguments.is_empty()
            || status.arguments.len() != 1
            || !is_path(
                &status.arguments[0],
                &self
                    .delivery
                    .selection
                    .open_variant
                    .path
                    .iter()
                    .map(|name| name.text.as_str())
                    .collect::<Vec<_>>(),
            )
        {
            return false;
        }
        let Some(Expression::Name(due)) = fields.get(due_name.as_str()).copied() else {
            return false;
        };
        self.parameter_field(callable, due)
            .is_some_and(|(_, field)| !field.optional && self.due_reference(&field.field_type))
    }
    fn patch_matches(&self, update: &UpdateExpression, callable: &CallableDeclaration) -> bool {
        if !self.exact_entity(&update.target)
            || update.transition.is_some()
            || !update.changes.is_empty()
            || update.conditional_changes.len() != 1
        {
            return false;
        }
        let Some(patch) = &update.patch else {
            return false;
        };
        let [patch_parameter] = patch.path.as_slice() else {
            return false;
        };
        let change = &update.conditional_changes[0];
        let Some((parameter, field)) = self.parameter_field(callable, &change.supplied) else {
            return false;
        };
        let Some(record) = self.record(&parameter.parameter_type) else {
            return false;
        };
        let selected = &self.delivery.hooks.supplied.path;
        if selected.len() != 2
            || record.name.text != selected[0].text
            || field.name.text != selected[1].text
            || parameter.name.text != patch_parameter.text
            || !field.optional
            || !self.due_reference(&field.field_type)
            || change.change.name.text != self.delivery.selection.unsent.path.last().unwrap().text
            || !is_none(&change.change.value)
        {
            return false;
        }
        record.fields.iter().all(|field| {
            !self.bound_field(&field.name.text)
                || field.name.text == self.delivery.selection.due.path.last().unwrap().text
        })
    }
    fn block(&mut self, block: &Block, parameters: &[Parameter], source: &str) {
        let parameters = parameters
            .iter()
            .filter(|parameter| !parameter_written(block, &parameter.name.text))
            .cloned()
            .collect::<Vec<_>>();
        let parameters = parameters.as_slice();
        for statement in &block.statements {
            match statement {
                Statement::Binding(statement) => {
                    self.expression(&statement.value, parameters, source)
                }
                Statement::Assignment(statement) => {
                    self.expression(&statement.value, parameters, source)
                }
                Statement::Return(statement) => {
                    self.expression(&statement.value, parameters, source)
                }
                Statement::Reject(statement) => {
                    for field in &statement.values {
                        self.expression(&field.value, parameters, source);
                    }
                }
                Statement::If(statement) => {
                    self.expression(&statement.condition, parameters, source);
                    self.block(&statement.then_block, parameters, source);
                    if let Some(block) = &statement.else_block {
                        self.block(block, parameters, source);
                    }
                }
                Statement::Match(statement) => {
                    self.expression(&statement.subject, parameters, source);
                    for arm in &statement.arms {
                        self.block(&arm.body, parameters, source);
                    }
                }
                Statement::Assert(statement) => {
                    self.expression(&statement.condition, parameters, source)
                }
                Statement::AdvanceClock(statement) => {
                    self.expression(&statement.duration, parameters, source)
                }
                Statement::Unsupported(_) => self.reject(source, statement.range()),
            }
        }
    }
    fn expression(&mut self, expression: &Expression, parameters: &[Parameter], source: &str) {
        let mut children = Vec::new();
        let mut fields = Vec::new();
        let mut rejections = Vec::new();
        match expression {
            Expression::Create(create) => {
                if self.exact_entity(&create.target)
                    && !self.allowed.contains(&(
                        source.to_owned(),
                        create.range.start,
                        create.range.end,
                    ))
                {
                    self.reject(source, create.range);
                }
                fields.extend(&create.fields);
                rejections.extend(create.conflicts.iter().map(|conflict| &conflict.rejection));
            }
            Expression::Update(update) => {
                if self.exact_entity(&update.target)
                    && !self.allowed.contains(&(
                        source.to_owned(),
                        update.range.start,
                        update.range.end,
                    ))
                {
                    let explicit_bound = update
                        .changes
                        .iter()
                        .chain(
                            update
                                .conditional_changes
                                .iter()
                                .map(|change| &change.change),
                        )
                        .any(|field| self.bound_field(&field.name.text));
                    let patch_bound = update.patch.as_ref().is_some_and(|patch| {
                        let [name] = patch.path.as_slice() else {
                            return true;
                        };
                        let Some(parameter) = parameters
                            .iter()
                            .find(|parameter| parameter.name.text == name.text)
                        else {
                            return true;
                        };
                        self.record(&parameter.parameter_type)
                            .map_or(true, |record| {
                                record
                                    .fields
                                    .iter()
                                    .any(|field| self.bound_field(&field.name.text))
                            })
                    });
                    let transition_bound = update.transition.as_ref().is_some_and(|transition| {
                        self.records
                            .get(self.delivery.selection.entity.text.as_str())
                            .and_then(|record| record.dossier.as_ref())
                            .and_then(|dossier| dossier.lifecycle.as_ref())
                            .and_then(|lifecycle| {
                                lifecycle
                                    .transitions
                                    .iter()
                                    .find(|candidate| candidate.name.text == transition.text)
                            })
                            .map_or(true, |transition| {
                                transition
                                    .set
                                    .iter()
                                    .any(|field| self.bound_field(&field.name.text))
                            })
                    });
                    if explicit_bound || patch_bound || transition_bound {
                        self.reject(source, update.range);
                    }
                }
                children.push(update.value.as_ref());
                fields.extend(&update.changes);
                fields.extend(
                    update
                        .conditional_changes
                        .iter()
                        .map(|change| &change.change),
                );
                rejections.extend(update.empty.as_ref());
                rejections.push(&update.missing);
                rejections.extend(update.conflicts.iter().map(|conflict| &conflict.rejection));
            }
            Expression::Delete(delete) => {
                children.push(delete.value.as_ref());
                rejections.push(&delete.missing);
                rejections.extend(delete.conflicts.iter().map(|conflict| &conflict.rejection));
            }
            Expression::Invocation(invocation) => {
                children.extend(&invocation.arguments);
                fields.extend(&invocation.named_arguments);
            }
            Expression::TestCall(call) => {
                children.extend(&call.invocation.arguments);
                fields.extend(&call.invocation.named_arguments);
            }
            Expression::Construction(construction) => fields.extend(&construction.fields),
            Expression::Object(object) => fields.extend(&object.fields),
            Expression::Query(query) => {
                children.push(query.value.as_ref());
                if let Some(pagination) = &query.pagination {
                    children.extend([pagination.limit.as_ref(), pagination.offset.as_ref()]);
                }
                for include in &query.includes {
                    children.extend([
                        include.pagination.limit.as_ref(),
                        include.pagination.offset.as_ref(),
                    ]);
                }
                if let Some(page) = &query.page {
                    children.extend(
                        page.predicates
                            .iter()
                            .map(|predicate| predicate.value.as_ref()),
                    );
                    children.extend([page.after.as_ref(), page.limit.as_ref()]);
                }
                rejections.extend(query.missing.as_ref());
            }
            Expression::Attempt(attempt) => children.push(attempt.value.as_ref()),
            Expression::Grouped(grouped) => children.push(grouped.value.as_ref()),
            Expression::Unary(unary) => children.push(unary.value.as_ref()),
            Expression::Binary(binary) => {
                children.extend([binary.left.as_ref(), binary.right.as_ref()])
            }
            Expression::OutcomeMatch(outcome) => {
                self.expression(&outcome.subject, parameters, source);
                for arm in &outcome.arms {
                    // Success introduces a new value origin, even if its name
                    // collides with a disjoint outer parameter. Do not borrow
                    // the outer shape as a patch footprint for the arm value.
                    let scoped = parameters.iter().filter(|parameter| {
                        !matches!(&arm.pattern, OutcomeMatchPattern::Success(binding) if binding.text == parameter.name.text)
                    }).cloned().collect::<Vec<_>>();
                    match &arm.body {
                        OutcomeMatchArmBody::Value(value) => {
                            self.expression(value, &scoped, source)
                        }
                        OutcomeMatchArmBody::Reject(reject) => {
                            for field in &reject.values {
                                self.expression(&field.value, &scoped, source);
                            }
                        }
                        OutcomeMatchArmBody::Propagate(_) => {}
                    }
                }
            }
            Expression::Literal(_) | Expression::Name(_) | Expression::Missing(_) => {}
        }
        for rejection in rejections {
            fields.extend(&rejection.values);
        }
        children.extend(fields.iter().map(|field| &field.value));
        for child in children {
            self.expression(child, parameters, source);
        }
    }
}

fn parameter_written(block: &Block, name: &str) -> bool {
    block.statements.iter().any(|statement| match statement {
        Statement::Binding(binding) => binding.name.text == name,
        Statement::Assignment(assignment) => assignment.target.text == name,
        Statement::If(branch) => {
            parameter_written(&branch.then_block, name)
                || branch
                    .else_block
                    .as_ref()
                    .is_some_and(|block| parameter_written(block, name))
        }
        Statement::Match(statement) => statement.arms.iter().any(|arm| {
            let shadowed = match &arm.pattern {
                MatchPattern::OptionalSome(pattern) => pattern.binding.text == name,
                MatchPattern::Variant(pattern) => {
                    pattern.bindings.iter().any(|binding| binding.text == name)
                }
                _ => false,
            };
            shadowed || parameter_written(&arm.body, name)
        }),
        _ => false,
    })
}
