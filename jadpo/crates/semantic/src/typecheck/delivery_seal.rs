//! Bound delivery types are compiler-owned. This rejects source containment and
//! typed values, not just obvious constructor spelling; unbound services retain
//! their ordinary language semantics. A seal is not runtime authority.
use super::{joined_name, resolved_type_value, TypeChecker, TypeValue};
use jadpo_syntax::{Declaration, FieldDeclaration, ParsedSyntax, RecordKind, TypeReference};
use std::collections::BTreeSet;

pub(super) struct DeliverySeal {
    pub(super) intent: String,
    pub(super) request: String,
    pub(super) receipt: String,
    pub(super) operation: String,
}

pub(super) fn seals_from_files(files: &[ParsedSyntax]) -> Vec<DeliverySeal> {
    files
        .iter()
        .flat_map(|file| &file.file.declarations)
        .filter_map(|declaration| {
            let Declaration::Job(job) = declaration else {
                return None;
            };
            let delivery = job.delivery.as_ref()?;
            Some(DeliverySeal {
                intent: delivery.service.intent.text.clone(),
                request: delivery.service.input.text.clone(),
                receipt: delivery.service.output.text.clone(),
                operation: joined_name(&delivery.service.operation.path),
            })
        })
        .collect()
}

impl TypeChecker<'_> {
    pub(super) fn delivery_type_is_sealed(&self, value: &TypeValue) -> bool {
        if self.delivery_seals.is_empty() {
            return false;
        }
        self.delivery_type_contains_seal(value, &mut BTreeSet::new())
    }

    fn delivery_type_contains_seal(
        &self,
        value: &TypeValue,
        visited: &mut BTreeSet<String>,
    ) -> bool {
        if value
            .arguments
            .iter()
            .any(|argument| self.delivery_type_contains_seal(argument, visited))
        {
            return true;
        }
        if !visited.insert(value.name.clone()) {
            return false;
        }
        // Effective field refinements and declared nominal ancestry are checked
        // by graph identity, not display/prefix comparison or primitive roots.
        let mut names = self.ancestors(&value.name);
        if names.is_empty() {
            names.push(value.name.clone());
        }
        names.iter().any(|name| {
            self.delivery_seals
                .iter()
                .any(|seal| name == &seal.intent || name == &seal.request || name == &seal.receipt)
                || self.catalogue.records.get(name).is_some_and(|fields| {
                    fields.values().any(|field| {
                        self.delivery_type_contains_seal(&field.declared_type, visited)
                    })
                })
                || self.catalogue.enums.get(name).is_some_and(|variants| {
                    variants
                        .values()
                        .flat_map(|fields| fields.values())
                        .any(|field| {
                            self.delivery_type_contains_seal(&field.declared_type, visited)
                        })
                })
        })
    }

    fn reject_sealed_reference(&mut self, reference: &TypeReference, source: &str) {
        if self.delivery_type_is_sealed(&resolved_type_value(reference, self.graph)) {
            self.push_diagnostic("TYPE_JOB_DELIVERY_SEALED_USE", source, reference.range);
        }
    }

    fn reject_sealed_fields(&mut self, fields: &[FieldDeclaration], source: &str) {
        for field in fields {
            self.reject_sealed_reference(&field.field_type, source);
            if let Some(reference) = &field.reference {
                self.reject_sealed_reference(&reference.target, source);
            }
        }
    }

    pub(super) fn check_delivery_sealed_declarations(&mut self, files: &[ParsedSyntax]) {
        if self.delivery_seals.is_empty() {
            return;
        }
        for file in files {
            let source = &file.source_name;
            for declaration in &file.file.declarations {
                match declaration {
                    Declaration::Type(declaration) => {
                        self.reject_sealed_reference(&declaration.parent, source)
                    }
                    Declaration::Record(record) => {
                        for field in &record.fields {
                            // Only the exact selected value request's required
                            // nominal intent key is a permitted sealed field use.
                            let allowed_key = record.kind == RecordKind::Value
                                && field.name.text == "idempotency_key"
                                && !field.optional
                                && !field.field_type.nullable
                                && field.field_type.arguments.is_empty()
                                && self.delivery_seals.iter().any(|seal| {
                                    record.name.text == seal.request
                                        && joined_name(&field.field_type.path) == seal.intent
                                });
                            if !allowed_key {
                                self.reject_sealed_reference(&field.field_type, source);
                            }
                            if let Some(reference) = &field.reference {
                                self.reject_sealed_reference(&reference.target, source);
                            }
                        }
                        for inverse in &record.inverses {
                            self.reject_sealed_reference(&inverse.via, source);
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
                                self.check_delivery_expression_type_slots(&field.value, source);
                            }
                            if let Some(visible) = &lifecycle.visible {
                                self.check_delivery_expression_type_slots(visible, source);
                            }
                            for transition in &lifecycle.transitions {
                                self.check_delivery_expression_type_slots(&transition.from, source);
                            }
                            if let Some(purge) = &lifecycle.purge {
                                self.check_delivery_expression_type_slots(&purge.after, source);
                            }
                        }
                    }
                    Declaration::Enum(declaration) => {
                        for variant in &declaration.variants {
                            self.reject_sealed_fields(&variant.fields, source);
                        }
                    }
                    Declaration::Failure(declaration) => {
                        self.reject_sealed_fields(&declaration.public_fields, source);
                        self.reject_sealed_fields(&declaration.internal_fields, source);
                    }
                    Declaration::Callable(callable) => {
                        for parameter in &callable.parameters {
                            self.reject_sealed_reference(&parameter.parameter_type, source);
                        }
                        self.reject_sealed_reference(&callable.return_type, source);
                        self.check_delivery_sealed_block(&callable.body, source);
                    }
                    Declaration::Route(route) => {
                        self.reject_sealed_fields(&route.path_fields, source);
                        for reference in [&route.input, &route.query, &route.output]
                            .into_iter()
                            .flatten()
                        {
                            self.reject_sealed_reference(reference, source);
                        }
                        for header in &route.headers {
                            self.reject_sealed_reference(&header.field_type, source);
                        }
                        if let Some(run) = &route.run {
                            self.check_delivery_expression_type_slots(
                                &jadpo_syntax::Expression::Invocation(run.clone()),
                                source,
                            );
                        }
                        if let Some(action) = &route.inline_action {
                            self.check_delivery_sealed_block(&action.body, source);
                        }
                    }
                    Declaration::Principal(principal) => {
                        for variant in &principal.variants {
                            self.reject_sealed_fields(&variant.fields, source);
                        }
                    }
                    Declaration::Config(config) => {
                        for field in &config.fields {
                            self.reject_sealed_reference(&field.field_type, source);
                        }
                    }
                    Declaration::Application(application) => {
                        self.reject_sealed_reference(&application.authentication.principal, source)
                    }
                    Declaration::Test(test) => self.check_delivery_sealed_block(&test.body, source),
                    Declaration::Job(job) => {
                        if let Some(run) = &job.run {
                            self.check_delivery_expression_type_slots(
                                &jadpo_syntax::Expression::Invocation(run.clone()),
                                source,
                            );
                        }
                    }
                    Declaration::Fixture(fixture) => {
                        if let Some(clock) = &fixture.clock {
                            self.check_delivery_expression_type_slots(clock, source);
                        }
                        for field in fixture.configuration.iter().flatten() {
                            self.check_delivery_expression_type_slots(&field.value, source);
                        }
                        for outcome in fixture.service_fakes.iter().flat_map(|fake| &fake.outcomes)
                        {
                            if let jadpo_syntax::FixtureServiceFakeValue::Accepted(value) =
                                &outcome.value
                            {
                                self.check_delivery_expression_type_slots(value, source);
                            }
                        }
                    }
                    Declaration::AuthenticationStrategy(strategy) => {
                        for validator in &strategy.validators {
                            for setting in &validator.settings {
                                self.check_delivery_expression_type_slots(&setting.value, source);
                            }
                            if let Some(credentials) = &validator.credentials {
                                self.check_delivery_expression_type_slots(
                                    &credentials.active,
                                    source,
                                );
                            }
                        }
                        for resolution in &strategy.resolutions {
                            self.check_delivery_expression_type_slots(&resolution.active, source);
                        }
                    }
                    Declaration::Locales(_) => {}
                }
            }
        }
        // A second checked service cannot reuse these sealed contract shapes.
        let invalid = self
            .graph
            .external_effects
            .iter()
            .filter(|effect| {
                let operation = format!("{}.{}", effect.service, effect.operation);
                !self
                    .delivery_seals
                    .iter()
                    .any(|seal| seal.operation == operation)
                    && [&effect.input, &effect.output, &effect.idempotency_type]
                        .into_iter()
                        .any(|name| self.delivery_type_is_sealed(&super::simple_type(name)))
            })
            .map(|effect| (effect.source.clone(), effect.range))
            .collect::<Vec<_>>();
        for (source, range) in invalid {
            self.push_diagnostic("TYPE_JOB_DELIVERY_SEALED_USE", &source, range);
        }
    }

    fn check_delivery_sealed_block(&mut self, block: &jadpo_syntax::Block, source: &str) {
        use jadpo_syntax::Statement;
        for statement in &block.statements {
            match statement {
                Statement::Binding(binding) => {
                    if let Some(annotation) = &binding.annotation {
                        self.reject_sealed_reference(annotation, source);
                    }
                    self.check_delivery_expression_type_slots(&binding.value, source);
                }
                Statement::Assignment(statement) => {
                    self.check_delivery_expression_type_slots(&statement.value, source)
                }
                Statement::Return(statement) => {
                    self.check_delivery_expression_type_slots(&statement.value, source)
                }
                Statement::Reject(statement) => {
                    for field in &statement.values {
                        self.check_delivery_expression_type_slots(&field.value, source);
                    }
                }
                Statement::If(statement) => {
                    self.check_delivery_expression_type_slots(&statement.condition, source);
                    self.check_delivery_sealed_block(&statement.then_block, source);
                    if let Some(block) = &statement.else_block {
                        self.check_delivery_sealed_block(block, source);
                    }
                }
                Statement::Match(statement) => {
                    self.check_delivery_expression_type_slots(&statement.subject, source);
                    for arm in &statement.arms {
                        self.check_delivery_sealed_block(&arm.body, source);
                    }
                }
                Statement::Assert(statement) => {
                    self.check_delivery_expression_type_slots(&statement.condition, source)
                }
                Statement::AdvanceClock(statement) => {
                    self.check_delivery_expression_type_slots(&statement.duration, source)
                }
                Statement::Unsupported(_) => {}
            }
        }
    }

    // Inferred values/calls are checked at their normal inference site. This
    // exhaustive AST walk additionally covers type slots such as query include/
    // page projections and nested local annotations, even in unused branches.
    fn check_delivery_expression_type_slots(
        &mut self,
        expression: &jadpo_syntax::Expression,
        source: &str,
    ) {
        use jadpo_syntax::{Expression as E, OutcomeMatchArmBody};
        let mut children = Vec::new();
        let mut fields = Vec::new();
        let mut rejects = Vec::new();
        match expression {
            E::Invocation(invocation) => {
                self.reject_delivery_expression_target(&invocation.callee, source);
                children.extend(&invocation.arguments);
                fields.extend(&invocation.named_arguments);
            }
            E::TestCall(call) => {
                self.reject_delivery_expression_target(&call.invocation.callee, source);
                children.extend(&call.invocation.arguments);
                fields.extend(&call.invocation.named_arguments);
            }
            E::Object(object) => fields.extend(&object.fields),
            E::Construction(construction) => {
                self.reject_delivery_expression_target(&construction.target, source);
                fields.extend(&construction.fields);
            }
            E::Create(create) => {
                fields.extend(&create.fields);
                rejects.extend(create.conflicts.iter().map(|c| &c.rejection));
            }
            E::Query(query) => {
                children.push(query.value.as_ref());
                if let Some(pagination) = &query.pagination {
                    children.extend([pagination.limit.as_ref(), pagination.offset.as_ref()]);
                }
                for include in &query.includes {
                    self.reject_sealed_reference(&include.result, source);
                    children.extend([
                        include.pagination.limit.as_ref(),
                        include.pagination.offset.as_ref(),
                    ]);
                }
                if let Some(page) = &query.page {
                    for reference in [&page.result, &page.projection, &page.cursor] {
                        self.reject_sealed_reference(reference, source);
                    }
                    children.extend(
                        page.predicates
                            .iter()
                            .map(|predicate| predicate.value.as_ref()),
                    );
                    children.extend([page.after.as_ref(), page.limit.as_ref()]);
                }
                rejects.extend(query.missing.as_ref());
            }
            E::Update(update) => {
                children.push(update.value.as_ref());
                fields.extend(&update.changes);
                fields.extend(
                    update
                        .conditional_changes
                        .iter()
                        .map(|change| &change.change),
                );
                rejects.extend(update.empty.as_ref());
                rejects.push(&update.missing);
                rejects.extend(update.conflicts.iter().map(|c| &c.rejection));
            }
            E::Delete(delete) => {
                children.push(delete.value.as_ref());
                rejects.push(&delete.missing);
                rejects.extend(delete.conflicts.iter().map(|c| &c.rejection));
            }
            E::Attempt(attempt) => children.push(attempt.value.as_ref()),
            E::Grouped(grouped) => children.push(grouped.value.as_ref()),
            E::Unary(unary) => children.push(unary.value.as_ref()),
            E::Binary(binary) => children.extend([binary.left.as_ref(), binary.right.as_ref()]),
            E::OutcomeMatch(outcome) => {
                children.push(outcome.subject.as_ref());
                for arm in &outcome.arms {
                    match &arm.body {
                        OutcomeMatchArmBody::Value(value) => children.push(value),
                        OutcomeMatchArmBody::Reject(reject) => rejects.push(reject),
                        OutcomeMatchArmBody::Propagate(_) => {}
                    }
                }
            }
            E::Literal(_) | E::Name(_) | E::Missing(_) => {}
        }
        for reject in rejects {
            fields.extend(&reject.values);
        }
        children.extend(fields.iter().map(|field| &field.value));
        for child in children {
            self.check_delivery_expression_type_slots(child, source);
        }
    }

    // An invalid enclosing invocation can prevent ordinary inference of an
    // extra argument. Still inspect every authored origin by resolved target
    // identity; malformed or erased arguments do not exempt constructors/calls.
    fn reject_delivery_expression_target(
        &mut self,
        target: &jadpo_syntax::NameExpression,
        source: &str,
    ) {
        let name = joined_name(&target.path);
        let sealed_constructor = self
            .graph
            .node(&name)
            .is_some_and(|node| node.kind.is_type())
            && self.delivery_type_is_sealed(&super::simple_type(&name));
        let sealed_operation = self.graph.node(&name).is_some_and(|node| {
            node.kind == super::NodeKind::ServiceOperation
                && self
                    .delivery_seals
                    .iter()
                    .any(|seal| seal.operation == name)
        });
        if sealed_constructor || sealed_operation {
            self.push_diagnostic("TYPE_JOB_DELIVERY_SEALED_USE", source, target.range);
        }
    }
}
