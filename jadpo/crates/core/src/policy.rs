use jadpo_diagnostics::{Diagnostic, DiagnosticFact, SourceSpan};
use jadpo_semantic::TypeCheckResult;
use jadpo_syntax::{
    Block, Declaration, Expression, NameExpression, ParsedSyntax, PolicyDeclaration, PolicyEffect,
    RecordDeclaration, RecordKind, Statement, TextRange,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PolicyModel {
    pub active: bool,
    pub bindings: Vec<RoleBindingContract>,
    pub memberships: Vec<MembershipContract>,
    pub entities: Vec<EntityPolicyContract>,
    pub operations: Vec<OperationPolicyContract>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoleBindingContract {
    pub entity: String,
    pub field: String,
    pub role: String,
    pub scope: String,
    pub principal: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MembershipContract {
    pub entity: String,
    pub scope_field: Option<String>,
    pub scope: String,
    pub member_field: String,
    pub principal: String,
    pub role_field: String,
    pub role_type: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityPolicyContract {
    pub entity: String,
    pub scope_field: Option<String>,
    pub scope: Option<String>,
    pub rules: Vec<PolicyRuleContract>,
    pub fields: Vec<FieldPolicyContract>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldPolicyContract {
    pub field: String,
    pub rules: Vec<PolicyRuleContract>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyRuleContract {
    pub subject: String,
    pub effects: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationPolicyContract {
    pub operation: String,
    pub obligations: Vec<PolicyObligation>,
    pub field_reads: Vec<PolicyFieldRead>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyObligation {
    pub entity: String,
    pub effect: String,
    pub subjects: Vec<String>,
    pub source: &'static str,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PolicyFieldRead {
    pub entity: String,
    pub field: String,
    pub subjects: Vec<String>,
    pub source: &'static str,
}

#[derive(Clone, Debug, Default)]
struct OperationEffects {
    entities: BTreeMap<String, BTreeSet<PolicyEffect>>,
    calls: BTreeSet<String>,
    invokes: BTreeSet<String>,
}

#[derive(Clone, Debug)]
struct LocatedRecord<'a> {
    record: &'a RecordDeclaration,
    source: &'a str,
}

pub fn analyze_policy(files: &[ParsedSyntax], typing: &TypeCheckResult) -> PolicyModel {
    let active = files.iter().any(|file| {
        file.file.declarations.iter().any(|declaration| {
            matches!(declaration, Declaration::Record(record) if record.policy.is_some() || record.membership.is_some() || record.fields.iter().any(|field| field.role.is_some() || field.policy.is_some()))
                || matches!(declaration, Declaration::Callable(callable) if callable.policy.is_some())
        })
    });
    if !active {
        return PolicyModel::default();
    }

    let mut model = PolicyModel {
        active: true,
        ..PolicyModel::default()
    };
    let mut enums = BTreeMap::<String, BTreeSet<String>>::new();
    let mut records = BTreeMap::<String, LocatedRecord<'_>>::new();
    for file in files {
        for declaration in &file.file.declarations {
            match declaration {
                Declaration::Enum(value) => {
                    enums.insert(
                        value.name.text.clone(),
                        value
                            .variants
                            .iter()
                            .map(|variant| variant.name.text.clone())
                            .collect(),
                    );
                }
                Declaration::Record(record) if record.kind == RecordKind::Entity => {
                    records.insert(
                        record.name.text.clone(),
                        LocatedRecord {
                            record,
                            source: &file.source_name,
                        },
                    );
                }
                _ => {}
            }
        }
    }

    let mut role_scopes = BTreeMap::<String, BTreeSet<String>>::new();
    for (entity_name, located) in &records {
        for field in &located.record.fields {
            let Some(binding) = &field.role else { continue };
            let role = joined(&binding.role);
            if !valid_role(&binding.role, &enums) {
                push(
                    &mut model.diagnostics,
                    Diagnostic::error("POLICY_BINDING_INVALID")
                        .with_fact(DiagnosticFact::Name(role.clone())),
                    located.source,
                    binding.range,
                );
                continue;
            }
            if !field.immutable {
                push(
                    &mut model.diagnostics,
                    Diagnostic::error("POLICY_ROLE_FIELD_UPDATE_FORBIDDEN").with_fact(
                        DiagnosticFact::Field(format!("{entity_name}.{}", field.name.text)),
                    ),
                    located.source,
                    field.range,
                );
            }
            let principal = referenced_entity(field).or_else(|| {
                located
                    .record
                    .dossier
                    .as_ref()
                    .is_some_and(|dossier| dossier.identity.text == field.name.text)
                    .then(|| entity_name.clone())
            });
            let Some(principal) = principal else {
                push(
                    &mut model.diagnostics,
                    Diagnostic::error("POLICY_BINDING_INVALID").with_fact(DiagnosticFact::Field(
                        format!("{entity_name}.{}", field.name.text),
                    )),
                    located.source,
                    field.range,
                );
                continue;
            };
            let role_type = binding.role.path[0].text.clone();
            role_scopes
                .entry(role_type)
                .or_default()
                .insert(entity_name.clone());
            model.bindings.push(RoleBindingContract {
                entity: entity_name.clone(),
                field: field.name.text.clone(),
                role,
                scope: entity_name.clone(),
                principal,
            });
        }

        if let Some(membership) = &located.record.membership {
            let application_scope = membership.scope.text == "application";
            let scope = (!application_scope)
                .then(|| field_by_name(located.record, &membership.scope.text))
                .flatten();
            let member = field_by_name(located.record, &membership.member.text);
            let role = field_by_name(located.record, &membership.role.text);
            let (Some(member), Some(role)) = (member, role) else {
                push(
                    &mut model.diagnostics,
                    Diagnostic::error("POLICY_BINDING_INVALID")
                        .with_fact(DiagnosticFact::Name(entity_name.clone())),
                    located.source,
                    membership.range,
                );
                continue;
            };
            let scope_entity = if application_scope {
                Some("application".to_owned())
            } else {
                scope.and_then(referenced_entity)
            };
            let member_entity = referenced_entity(member);
            let role_type = type_name(&role.field_type);
            if scope_entity.is_none()
                || member_entity.is_none()
                || !enums.contains_key(&role_type)
                || !located.record.is_persistent_entity()
            {
                push(
                    &mut model.diagnostics,
                    Diagnostic::error("POLICY_BINDING_INVALID")
                        .with_fact(DiagnosticFact::Name(entity_name.clone())),
                    located.source,
                    membership.range,
                );
                continue;
            }
            let scope_entity = scope_entity.expect("validated scope");
            let member_entity = member_entity.expect("validated member");
            role_scopes
                .entry(role_type.clone())
                .or_default()
                .insert(scope_entity.clone());
            model.memberships.push(MembershipContract {
                entity: entity_name.clone(),
                scope_field: scope.map(|scope| scope.name.text.clone()),
                scope: scope_entity,
                member_field: member.name.text.clone(),
                principal: member_entity,
                role_field: role.name.text.clone(),
                role_type,
            });
        }
    }

    for (entity_name, located) in &records {
        let Some(policy) = &located.record.policy else {
            continue;
        };
        let mut contract = EntityPolicyContract {
            entity: entity_name.clone(),
            scope_field: None,
            scope: None,
            rules: check_rules(policy, &enums, located.source, &mut model.diagnostics),
            fields: Vec::new(),
        };
        if contract
            .rules
            .iter()
            .any(|rule| rule.effects.iter().any(|effect| effect == "invoke"))
        {
            push(
                &mut model.diagnostics,
                Diagnostic::error("POLICY_INVOKE_CONTEXT")
                    .with_fact(DiagnosticFact::Operation(entity_name.clone())),
                located.source,
                policy.range,
            );
        }
        validate_rule_duplicates(policy, located.source, &mut model.diagnostics);

        let scoped_role_types = policy
            .rules
            .iter()
            .chain(
                policy
                    .operations
                    .iter()
                    .flat_map(|operation| &operation.rules),
            )
            .filter(|rule| !is_access_subject(&rule.subject))
            .filter_map(|rule| rule.subject.path.first().map(|part| part.text.clone()))
            .collect::<BTreeSet<_>>();
        let mut selected_scope = None::<(String, String)>;
        for role_type in scoped_role_types {
            let Some(scopes) = role_scopes.get(&role_type) else {
                push(
                    &mut model.diagnostics,
                    Diagnostic::error("POLICY_ROLE_UNBOUND")
                        .with_fact(DiagnosticFact::Name(role_type.clone())),
                    located.source,
                    policy.range,
                );
                continue;
            };
            if scopes.len() == 1 && scopes.contains("application") {
                // Application memberships are resolved independently of the
                // entity's resource scope, including exception-only roles.
                if contract.scope.is_none() {
                    contract.scope = Some("application".to_owned());
                }
                continue;
            }
            if scopes.len() != 1 {
                push(
                    &mut model.diagnostics,
                    Diagnostic::error("POLICY_BINDING_INVALID")
                        .with_fact(DiagnosticFact::Name(role_type.clone())),
                    located.source,
                    policy.range,
                );
                continue;
            }
            let scope_entity = scopes.iter().next().expect("one scope");
            let candidates = located
                .record
                .fields
                .iter()
                .filter(|field| referenced_entity(field).as_deref() == Some(scope_entity.as_str()))
                .map(|field| field.name.text.clone())
                .collect::<Vec<_>>();
            let chosen = if scope_entity == entity_name
                && model.bindings.iter().any(|binding| {
                    binding.entity == *entity_name && binding.role.starts_with(&role_type)
                }) {
                None
            } else if let Some(explicit) = &policy.scope {
                if candidates.contains(&explicit.text) {
                    Some(explicit.text.clone())
                } else {
                    push(
                        &mut model.diagnostics,
                        Diagnostic::error("POLICY_SCOPE_MISSING")
                            .with_fact(DiagnosticFact::Scope(explicit.text.clone())),
                        located.source,
                        explicit.range,
                    );
                    None
                }
            } else if candidates.len() == 1 {
                Some(candidates[0].clone())
            } else if candidates.is_empty() {
                push(
                    &mut model.diagnostics,
                    Diagnostic::error("POLICY_SCOPE_MISSING")
                        .with_fact(DiagnosticFact::Name(scope_entity.clone())),
                    located.source,
                    policy.range,
                );
                None
            } else {
                push(
                    &mut model.diagnostics,
                    Diagnostic::error("POLICY_SCOPE_AMBIGUOUS")
                        .with_fact(DiagnosticFact::Name(scope_entity.clone())),
                    located.source,
                    policy.range,
                );
                None
            };
            if let Some(field) = chosen {
                if selected_scope
                    .as_ref()
                    .is_some_and(|(_, existing)| existing != &field)
                {
                    push(
                        &mut model.diagnostics,
                        Diagnostic::error("POLICY_SCOPE_AMBIGUOUS")
                            .with_fact(DiagnosticFact::Name(entity_name.clone())),
                        located.source,
                        policy.range,
                    );
                } else {
                    selected_scope = Some((scope_entity.clone(), field));
                }
            } else if scope_entity == entity_name {
                contract.scope = Some(entity_name.clone());
            }
        }
        if let Some((scope, field)) = selected_scope {
            contract.scope = Some(scope);
            contract.scope_field = Some(field);
        }

        for field in &located.record.fields {
            let Some(field_policy) = &field.policy else {
                continue;
            };
            validate_rule_duplicates(field_policy, located.source, &mut model.diagnostics);
            let rules = check_rules(field_policy, &enums, located.source, &mut model.diagnostics);
            for rule in &rules {
                let base = contract
                    .rules
                    .iter()
                    .find(|base| base.subject == rule.subject);
                for effect in &rule.effects {
                    let named_grant = policy.operations.iter().any(|operation| {
                        operation.rules.iter().any(|grant| {
                            joined(&grant.subject) == rule.subject
                                && grant.effects.iter().any(|grant| grant.as_str() == effect)
                        })
                    });
                    // Field permission still ANDs the exact operation grant at
                    // runtime. A named exception is not entity-wide authority.
                    if !base.is_some_and(|base| base.effects.contains(effect)) && !named_grant {
                        push(
                            &mut model.diagnostics,
                            Diagnostic::error("POLICY_FIELD_WIDENS_ENTITY")
                                .with_fact(DiagnosticFact::Field(format!(
                                    "{entity_name}.{}",
                                    field.name.text
                                )))
                                .with_fact(DiagnosticFact::Name(rule.subject.clone()))
                                .with_fact(DiagnosticFact::Operation(effect.clone())),
                            located.source,
                            field_policy.range,
                        );
                    }
                }
            }
            contract.fields.push(FieldPolicyContract {
                field: field.name.text.clone(),
                rules,
            });
        }
        model.entities.push(contract);
    }

    analyze_operation_policies(files, &records, &enums, typing, &mut model);

    model
        .bindings
        .sort_by(|a, b| (&a.entity, &a.field).cmp(&(&b.entity, &b.field)));
    model.memberships.sort_by(|a, b| a.entity.cmp(&b.entity));
    model.entities.sort_by(|a, b| a.entity.cmp(&b.entity));
    model
        .operations
        .sort_by(|a, b| a.operation.cmp(&b.operation));
    model
}

fn analyze_operation_policies(
    files: &[ParsedSyntax],
    records: &BTreeMap<String, LocatedRecord<'_>>,
    enums: &BTreeMap<String, BTreeSet<String>>,
    typing: &TypeCheckResult,
    model: &mut PolicyModel,
) {
    let mut callables = BTreeMap::new();
    let mut record_types = BTreeMap::new();
    let mut effects = BTreeMap::<String, OperationEffects>::new();
    for file in files {
        for declaration in &file.file.declarations {
            if let Declaration::Record(record) = declaration {
                record_types.insert(record.name.text.clone(), record);
            }
            if let Declaration::Callable(callable) = declaration {
                callables.insert(
                    callable.name.text.clone(),
                    (callable, file.source_name.as_str()),
                );
                let mut direct = OperationEffects::default();
                collect_block_effects(
                    &callable.body,
                    &mut direct,
                    model,
                    file.source_name.as_str(),
                );
                collect_include_effects_block(&callable.body, records, &mut direct);
                if callable.policy.is_some() {
                    direct.invokes.insert(callable.name.text.clone());
                }
                effects.insert(callable.name.text.clone(), direct);
            }
        }
    }
    let names = callables.keys().cloned().collect::<BTreeSet<_>>();
    for effect in effects.values_mut() {
        effect.calls = effect
            .calls
            .iter()
            .map(|name| resolve_call(name, &names))
            .collect();
    }
    loop {
        let snapshot = effects.clone();
        let mut changed = false;
        for effect in effects.values_mut() {
            for call in effect.calls.clone() {
                let Some(callee) = snapshot.get(&call) else {
                    continue;
                };
                for (entity, reached) in &callee.entities {
                    let target = effect.entities.entry(entity.clone()).or_default();
                    let before = target.len();
                    target.extend(reached.iter().copied());
                    changed |= target.len() != before;
                }
                let before = effect.invokes.len();
                effect.invokes.extend(callee.invokes.iter().cloned());
                changed |= effect.invokes.len() != before;
            }
        }
        if !changed {
            break;
        }
    }

    validate_operation_exceptions(records, enums, &callables, &effects, model);

    for (name, (callable, source)) in &callables {
        let Some(reached) = effects.get(name) else {
            continue;
        };
        let mut obligations = Vec::new();
        for (entity, entity_effects) in &reached.entities {
            for effect in entity_effects {
                let base = model
                    .entities
                    .iter()
                    .find(|policy| policy.entity == *entity);
                let exception = records
                    .get(entity)
                    .and_then(|record| record.record.policy.as_ref())
                    .and_then(|policy| {
                        policy.operations.iter().find(|operation| {
                            callable
                                .owner
                                .as_ref()
                                .is_some_and(|owner| owner.text == *entity)
                                && operation.name.text
                                    == name.rsplit('.').next().unwrap_or(name.as_str())
                        })
                    });
                let (subjects, policy_source) = if let Some(exception) = exception {
                    (
                        exception
                            .rules
                            .iter()
                            .filter(|rule| rule.effects.contains(effect))
                            .map(|rule| joined(&rule.subject))
                            .collect::<Vec<_>>(),
                        "operation_exception",
                    )
                } else {
                    (
                        base.into_iter()
                            .flat_map(|policy| &policy.rules)
                            .filter(|rule| {
                                rule.effects.iter().any(|grant| grant == effect.as_str())
                            })
                            .map(|rule| rule.subject.clone())
                            .collect::<Vec<_>>(),
                        "entity",
                    )
                };
                if subjects.is_empty() {
                    push(
                        &mut model.diagnostics,
                        Diagnostic::error("POLICY_EFFECT_UNGRANTED")
                            .with_fact(DiagnosticFact::Operation(name.clone()))
                            .with_fact(DiagnosticFact::Name(entity.clone()))
                            .with_fact(DiagnosticFact::Usage(effect.as_str().to_owned())),
                        source,
                        callable.range,
                    );
                }
                obligations.push(PolicyObligation {
                    entity: entity.clone(),
                    effect: effect.as_str().to_owned(),
                    subjects,
                    source: policy_source,
                });
            }
        }
        if let Some(policy) = &callable.policy {
            validate_rule_duplicates(policy, source, &mut model.diagnostics);
            let rules = check_rules(policy, enums, source, &mut model.diagnostics);
            if callable.kind == jadpo_syntax::CallableKind::Function
                || policy.scope.is_some()
                || !policy.operations.is_empty()
                || rules
                    .iter()
                    .any(|rule| rule.effects.iter().any(|effect| effect != "invoke"))
            {
                push(
                    &mut model.diagnostics,
                    Diagnostic::error("POLICY_INVOKE_CONTEXT")
                        .with_fact(DiagnosticFact::Operation(name.clone())),
                    source,
                    policy.range,
                );
            }
            for rule in &rules {
                if rule.subject.starts_with("Access.") {
                    continue;
                }
                let role_type = rule.subject.split('.').next().unwrap_or_default();
                if !model.bindings.iter().any(|binding| {
                    binding.role.starts_with(&format!("{role_type}."))
                        && binding.scope == "application"
                }) && !model.memberships.iter().any(|membership| {
                    membership.role_type == role_type && membership.scope == "application"
                }) {
                    push(
                        &mut model.diagnostics,
                        Diagnostic::error("POLICY_ROLE_UNBOUND")
                            .with_fact(DiagnosticFact::Name(role_type.to_owned())),
                        source,
                        policy.range,
                    );
                }
            }
            let subjects = rules
                .iter()
                .filter(|rule| rule.effects.iter().any(|effect| effect == "invoke"))
                .map(|rule| rule.subject.clone())
                .collect::<Vec<_>>();
            if subjects.is_empty() {
                push(
                    &mut model.diagnostics,
                    Diagnostic::error("POLICY_EFFECT_UNGRANTED")
                        .with_fact(DiagnosticFact::Operation(name.clone()))
                        .with_fact(DiagnosticFact::Usage("invoke".to_owned())),
                    source,
                    policy.range,
                );
            }
            obligations.push(PolicyObligation {
                entity: name.clone(),
                effect: "invoke".to_owned(),
                subjects,
                source: "operation",
            });
        } else if reached.entities.is_empty()
            && reached.invokes.is_empty()
            && callable.kind != jadpo_syntax::CallableKind::Function
        {
            push(
                &mut model.diagnostics,
                Diagnostic::error("POLICY_EFFECT_UNGRANTED")
                    .with_fact(DiagnosticFact::Operation(name.clone()))
                    .with_fact(DiagnosticFact::Usage("invoke".to_owned())),
                source,
                callable.range,
            );
        }
        if !obligations.is_empty() {
            validate_output_field_policy(callable, source, &obligations, &record_types, model);
            let field_reads = validate_typed_field_access(
                name,
                source,
                callable.range,
                &obligations,
                typing,
                model,
            );
            obligations.sort_by(|a, b| (&a.entity, &a.effect).cmp(&(&b.entity, &b.effect)));
            model.operations.push(OperationPolicyContract {
                operation: name.clone(),
                obligations,
                field_reads,
            });
        }
    }

    analyze_route_policies(files, records, &names, &record_types, typing, model);
    validate_public_routes(files, model);
}

fn validate_typed_field_access(
    operation: &str,
    source: &str,
    range: TextRange,
    obligations: &[PolicyObligation],
    typing: &TypeCheckResult,
    model: &mut PolicyModel,
) -> Vec<PolicyFieldRead> {
    let mut reads = BTreeSet::new();
    let restricted = model
        .entities
        .iter()
        .flat_map(|entity| {
            entity
                .fields
                .iter()
                .map(move |field| (entity.entity.as_str(), field))
        })
        .collect::<Vec<_>>();
    for expression in typing.expressions.iter().filter(|expression| {
        expression.source == source
            && expression.range.start >= range.start
            && expression.range.end <= range.end
    }) {
        let inferred = expression.type_name.trim_end_matches('?');
        let Some((entity_name, field)) = restricted
            .iter()
            .find(|(entity, field)| inferred == format!("{entity}.{}", field.field))
        else {
            continue;
        };
        let admitted = obligations
            .iter()
            .filter(|obligation| obligation.entity == *entity_name && obligation.effect == "read")
            .flat_map(|obligation| obligation.subjects.iter().cloned())
            .collect::<BTreeSet<_>>();
        let allowed = field
            .rules
            .iter()
            .filter(|rule| rule.effects.iter().any(|effect| effect == "read"))
            .map(|rule| rule.subject.clone())
            .collect::<BTreeSet<_>>();
        if admitted.is_empty() {
            continue;
        }
        if !admitted.is_subset(&allowed) {
            let unproved = admitted
                .difference(&allowed)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            push(
                &mut model.diagnostics,
                Diagnostic::error("POLICY_OUTPUT_FIELD_UNPROVED")
                    .with_fact(DiagnosticFact::Operation(operation.to_owned()))
                    .with_fact(DiagnosticFact::Field(format!(
                        "{entity_name}.{}",
                        field.field
                    )))
                    .with_fact(DiagnosticFact::Name(unproved)),
                source,
                expression.range,
            );
        }
        reads.insert(PolicyFieldRead {
            entity: (*entity_name).to_owned(),
            field: field.field.clone(),
            subjects: admitted.into_iter().collect(),
            source: "typed_expression",
        });
    }
    reads.into_iter().collect()
}

fn validate_output_field_policy(
    callable: &jadpo_syntax::CallableDeclaration,
    source: &str,
    obligations: &[PolicyObligation],
    records: &BTreeMap<String, &RecordDeclaration>,
    model: &mut PolicyModel,
) {
    validate_output_reference_policy(
        &callable.return_type,
        &callable.name.text,
        callable.return_annotation_range,
        source,
        obligations,
        records,
        model,
    );
}

fn validate_output_reference_policy(
    output: &jadpo_syntax::TypeReference,
    operation: &str,
    range: TextRange,
    source: &str,
    obligations: &[PolicyObligation],
    records: &BTreeMap<String, &RecordDeclaration>,
    model: &mut PolicyModel,
) {
    let mut selected = BTreeSet::new();
    collect_selected_policy_fields(output, records, model, &mut BTreeSet::new(), &mut selected);
    for (entity, field) in selected {
        let admitted = obligations
            .iter()
            .filter(|obligation| obligation.entity == entity && obligation.effect == "read")
            .flat_map(|obligation| obligation.subjects.iter().cloned())
            .collect::<BTreeSet<_>>();
        let allowed = model
            .entities
            .iter()
            .find(|policy| policy.entity == entity)
            .and_then(|policy| {
                policy
                    .fields
                    .iter()
                    .find(|candidate| candidate.field == field)
            })
            .into_iter()
            .flat_map(|policy| &policy.rules)
            .filter(|rule| rule.effects.iter().any(|effect| effect == "read"))
            .map(|rule| rule.subject.clone())
            .collect::<BTreeSet<_>>();
        if admitted.is_subset(&allowed) {
            continue;
        }
        let unproved = admitted
            .difference(&allowed)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        push(
            &mut model.diagnostics,
            Diagnostic::error("POLICY_OUTPUT_FIELD_UNPROVED")
                .with_fact(DiagnosticFact::Operation(operation.to_owned()))
                .with_fact(DiagnosticFact::Field(format!("{entity}.{field}")))
                .with_fact(DiagnosticFact::Name(unproved)),
            source,
            range,
        );
    }
}

fn collect_selected_policy_fields(
    reference: &jadpo_syntax::TypeReference,
    records: &BTreeMap<String, &RecordDeclaration>,
    model: &PolicyModel,
    visiting: &mut BTreeSet<String>,
    selected: &mut BTreeSet<(String, String)>,
) {
    let Some(root) = reference.path.first().map(|part| part.text.as_str()) else {
        return;
    };
    if reference.path.len() >= 2 {
        let field = &reference.path[1].text;
        if model.entities.iter().any(|policy| {
            policy.entity == root
                && policy
                    .fields
                    .iter()
                    .any(|protected| protected.field == *field)
        }) {
            selected.insert((root.to_owned(), field.clone()));
        }
        return;
    }
    if let Some(policy) = model.entities.iter().find(|policy| policy.entity == root) {
        selected.extend(
            policy
                .fields
                .iter()
                .map(|field| (root.to_owned(), field.field.clone())),
        );
        return;
    }
    if !visiting.insert(root.to_owned()) {
        return;
    }
    if let Some(record) = records.get(root) {
        for field in &record.fields {
            collect_selected_policy_fields(&field.field_type, records, model, visiting, selected);
        }
    }
    visiting.remove(root);
}

fn validate_operation_exceptions(
    records: &BTreeMap<String, LocatedRecord<'_>>,
    enums: &BTreeMap<String, BTreeSet<String>>,
    callables: &BTreeMap<String, (&jadpo_syntax::CallableDeclaration, &str)>,
    effects: &BTreeMap<String, OperationEffects>,
    model: &mut PolicyModel,
) {
    for (entity_name, located) in records {
        let Some(policy) = &located.record.policy else {
            continue;
        };
        for exception in &policy.operations {
            let operation = format!("{entity_name}.{}", exception.name.text);
            let Some((callable, source)) = callables.get(&operation) else {
                push(
                    &mut model.diagnostics,
                    Diagnostic::error("POLICY_OPERATION_UNKNOWN")
                        .with_fact(DiagnosticFact::Operation(operation)),
                    located.source,
                    exception.name.range,
                );
                continue;
            };
            let reached = effects
                .get(&callable.name.text)
                .and_then(|effects| effects.entities.get(entity_name))
                .cloned()
                .unwrap_or_default();
            for rule in &exception.rules {
                let subject = joined(&rule.subject);
                if !is_access_subject(&rule.subject) && !valid_role(&rule.subject, enums) {
                    push(
                        &mut model.diagnostics,
                        Diagnostic::error("POLICY_BINDING_INVALID")
                            .with_fact(DiagnosticFact::Name(subject)),
                        source,
                        rule.subject.range,
                    );
                }
                for effect in &rule.effects {
                    if !reached.contains(effect) {
                        push(
                            &mut model.diagnostics,
                            Diagnostic::error("POLICY_OPERATION_EFFECT_MISMATCH")
                                .with_fact(DiagnosticFact::Operation(callable.name.text.clone()))
                                .with_fact(DiagnosticFact::Name(effect.as_str().to_owned())),
                            source,
                            rule.range,
                        );
                    }
                }
            }
        }
    }
}

fn analyze_route_policies(
    files: &[ParsedSyntax],
    records: &BTreeMap<String, LocatedRecord<'_>>,
    names: &BTreeSet<String>,
    record_types: &BTreeMap<String, &RecordDeclaration>,
    typing: &TypeCheckResult,
    model: &mut PolicyModel,
) {
    for file in files {
        for declaration in &file.file.declarations {
            let Declaration::Route(route) = declaration else {
                continue;
            };
            let operation_name = format!("route:{}:{}", route_method(route.method), route.path);
            let mut obligations = Vec::new();
            let mut field_reads = Vec::new();
            if let Some(run) = &route.run {
                let callee = resolve_call(&joined(&run.callee), names);
                if let Some(contract) = model
                    .operations
                    .iter()
                    .find(|contract| contract.operation == callee)
                {
                    field_reads.extend(contract.field_reads.iter().cloned().map(|mut read| {
                        read.source = "transitive_operation";
                        read
                    }));
                    obligations.extend(contract.obligations.iter().cloned().map(
                        |mut obligation| {
                            obligation.source = "transitive_operation";
                            obligation
                        },
                    ));
                }
            }
            if let Some(action) = &route.inline_action {
                let mut reached = OperationEffects::default();
                collect_block_effects(&action.body, &mut reached, model, file.source_name.as_str());
                collect_include_effects_block(&action.body, records, &mut reached);
                for (entity, entity_effects) in reached.entities {
                    for effect in entity_effects {
                        let subjects = model
                            .entities
                            .iter()
                            .find(|policy| policy.entity == entity)
                            .into_iter()
                            .flat_map(|policy| &policy.rules)
                            .filter(|rule| {
                                rule.effects.iter().any(|grant| grant == effect.as_str())
                            })
                            .map(|rule| rule.subject.clone())
                            .collect::<Vec<_>>();
                        if subjects.is_empty() {
                            push(
                                &mut model.diagnostics,
                                Diagnostic::error("POLICY_EFFECT_UNGRANTED")
                                    .with_fact(DiagnosticFact::Operation(operation_name.clone()))
                                    .with_fact(DiagnosticFact::Name(entity.clone()))
                                    .with_fact(DiagnosticFact::Usage(effect.as_str().to_owned())),
                                &file.source_name,
                                route.range,
                            );
                        }
                        obligations.push(PolicyObligation {
                            entity: entity.clone(),
                            effect: effect.as_str().to_owned(),
                            subjects,
                            source: "route_inline",
                        });
                    }
                }
                for call in reached.calls {
                    let callee = resolve_call(&call, names);
                    if let Some(contract) = model
                        .operations
                        .iter()
                        .find(|contract| contract.operation == callee)
                    {
                        field_reads.extend(contract.field_reads.iter().cloned().map(|mut read| {
                            read.source = "transitive_operation";
                            read
                        }));
                        obligations.extend(contract.obligations.iter().cloned().map(
                            |mut obligation| {
                                obligation.source = "transitive_operation";
                                obligation
                            },
                        ));
                    }
                }
            }
            if let Some(output) = &route.output {
                validate_output_reference_policy(
                    output,
                    &operation_name,
                    output.range,
                    &file.source_name,
                    &obligations,
                    record_types,
                    model,
                );
            }
            if route.inline_action.is_some() {
                field_reads.extend(validate_typed_field_access(
                    &operation_name,
                    &file.source_name,
                    route.range,
                    &obligations,
                    typing,
                    model,
                ));
            }
            if !obligations.is_empty() {
                obligations.sort_by(|left, right| {
                    (&left.entity, &left.effect, &left.subjects).cmp(&(
                        &right.entity,
                        &right.effect,
                        &right.subjects,
                    ))
                });
                field_reads.sort();
                field_reads.dedup();
                model.operations.push(OperationPolicyContract {
                    operation: operation_name,
                    obligations,
                    field_reads,
                });
            }
        }
    }
}

fn validate_public_routes(files: &[ParsedSyntax], model: &mut PolicyModel) {
    for file in files {
        for declaration in &file.file.declarations {
            let Declaration::Route(route) = declaration else {
                continue;
            };
            if !route.public {
                continue;
            }
            let operation_name = format!("route:{}:{}", route_method(route.method), route.path);
            let Some(contract) = model
                .operations
                .iter()
                .find(|contract| contract.operation == operation_name)
            else {
                continue;
            };
            for obligation in &contract.obligations {
                if obligation
                    .subjects
                    .iter()
                    .any(|subject| subject == "Access.public")
                {
                    continue;
                }
                let diagnostic = Diagnostic::error("POLICY_PUBLIC_GRANT_MISSING")
                    .with_fact(DiagnosticFact::Route(format!(
                        "{} {}",
                        route_method(route.method),
                        route.path
                    )))
                    .with_fact(DiagnosticFact::Usage(obligation.effect.clone()));
                let diagnostic = if obligation.effect == "invoke" {
                    diagnostic.with_fact(DiagnosticFact::Operation(obligation.entity.clone()))
                } else {
                    diagnostic.with_fact(DiagnosticFact::Name(obligation.entity.clone()))
                };
                push(
                    &mut model.diagnostics,
                    diagnostic,
                    &file.source_name,
                    route.range,
                );
            }
        }
    }
}

fn collect_include_effects_block(
    block: &Block,
    records: &BTreeMap<String, LocatedRecord<'_>>,
    effects: &mut OperationEffects,
) {
    for statement in &block.statements {
        match statement {
            Statement::Binding(value) => {
                collect_include_effects_expression(&value.value, records, effects)
            }
            Statement::Assignment(value) => {
                collect_include_effects_expression(&value.value, records, effects)
            }
            Statement::Return(value) => {
                collect_include_effects_expression(&value.value, records, effects)
            }
            Statement::Reject(value) => {
                for field in &value.values {
                    collect_include_effects_expression(&field.value, records, effects);
                }
            }
            Statement::If(value) => {
                collect_include_effects_expression(&value.condition, records, effects);
                collect_include_effects_block(&value.then_block, records, effects);
                if let Some(block) = &value.else_block {
                    collect_include_effects_block(block, records, effects);
                }
            }
            Statement::Match(value) => {
                collect_include_effects_expression(&value.subject, records, effects);
                for arm in &value.arms {
                    collect_include_effects_block(&arm.body, records, effects);
                }
            }
            Statement::Assert(value) => {
                collect_include_effects_expression(&value.condition, records, effects)
            }
            Statement::AdvanceClock(value) => {
                collect_include_effects_expression(&value.duration, records, effects)
            }
            Statement::Unsupported(_) => {}
        }
    }
}

fn collect_include_effects_expression(
    expression: &Expression,
    records: &BTreeMap<String, LocatedRecord<'_>>,
    effects: &mut OperationEffects,
) {
    match expression {
        Expression::Query(query) => {
            let root = joined(&query.target);
            for include in &query.includes {
                let Some(child) = relationship_target(&root, &include.relationship.text, records)
                else {
                    continue;
                };
                effects
                    .entities
                    .entry(child.clone())
                    .or_default()
                    .insert(PolicyEffect::Read);
                if let Some(nested) = &include.nested_relationship {
                    if let Some(leaf) = relationship_target(&child, &nested.text, records) {
                        effects
                            .entities
                            .entry(leaf)
                            .or_default()
                            .insert(PolicyEffect::Read);
                    }
                }
            }
            collect_include_effects_expression(&query.value, records, effects);
        }
        Expression::Invocation(value) => {
            for argument in &value.arguments {
                collect_include_effects_expression(argument, records, effects);
            }
            for argument in &value.named_arguments {
                collect_include_effects_expression(&argument.value, records, effects);
            }
        }
        Expression::TestCall(value) => {
            for argument in &value.invocation.arguments {
                collect_include_effects_expression(argument, records, effects);
            }
            for argument in &value.invocation.named_arguments {
                collect_include_effects_expression(&argument.value, records, effects);
            }
        }
        Expression::Construction(value) => {
            for field in &value.fields {
                collect_include_effects_expression(&field.value, records, effects);
            }
        }
        Expression::Object(value) => {
            for field in &value.fields {
                collect_include_effects_expression(&field.value, records, effects);
            }
        }
        Expression::Create(value) => {
            for field in &value.fields {
                collect_include_effects_expression(&field.value, records, effects);
            }
        }
        Expression::Update(value) => {
            collect_include_effects_expression(&value.value, records, effects);
            for field in &value.changes {
                collect_include_effects_expression(&field.value, records, effects);
            }
            for field in &value.conditional_changes {
                collect_include_effects_expression(&field.change.value, records, effects);
            }
        }
        Expression::Delete(value) => {
            collect_include_effects_expression(&value.value, records, effects)
        }
        Expression::Attempt(value) => {
            collect_include_effects_expression(&value.value, records, effects)
        }
        Expression::OutcomeMatch(value) => {
            collect_include_effects_expression(&value.subject, records, effects);
            for arm in &value.arms {
                match &arm.body {
                    jadpo_syntax::OutcomeMatchArmBody::Value(value) => {
                        collect_include_effects_expression(value, records, effects)
                    }
                    jadpo_syntax::OutcomeMatchArmBody::Reject(value) => {
                        for field in &value.values {
                            collect_include_effects_expression(&field.value, records, effects);
                        }
                    }
                    jadpo_syntax::OutcomeMatchArmBody::Propagate(_) => {}
                }
            }
        }
        Expression::Unary(value) => {
            collect_include_effects_expression(&value.value, records, effects)
        }
        Expression::Binary(value) => {
            collect_include_effects_expression(&value.left, records, effects);
            collect_include_effects_expression(&value.right, records, effects);
        }
        Expression::Grouped(value) => {
            collect_include_effects_expression(&value.value, records, effects)
        }
        Expression::Literal(_) | Expression::Name(_) | Expression::Missing(_) => {}
    }
}

fn relationship_target(
    entity: &str,
    relationship: &str,
    records: &BTreeMap<String, LocatedRecord<'_>>,
) -> Option<String> {
    let record = records.get(entity)?.record;
    if let Some(inverse) = record
        .inverses
        .iter()
        .find(|inverse| inverse.name.text == relationship)
    {
        return Some(inverse.target.text.clone());
    }
    record.fields.iter().find_map(|field| {
        let reference = field.reference.as_ref()?;
        let authored = reference
            .relationship
            .as_ref()
            .map_or(field.name.text.as_str(), |name| name.text.as_str());
        (authored == relationship).then(|| reference.target.path[0].text.clone())
    })
}

fn collect_block_effects(
    block: &Block,
    effects: &mut OperationEffects,
    model: &mut PolicyModel,
    source: &str,
) {
    for statement in &block.statements {
        match statement {
            Statement::Binding(value) => {
                collect_expression_effects(&value.value, effects, model, source)
            }
            Statement::Assignment(value) => {
                collect_expression_effects(&value.value, effects, model, source)
            }
            Statement::Return(value) => {
                collect_expression_effects(&value.value, effects, model, source)
            }
            Statement::Reject(value) => {
                for field in &value.values {
                    collect_expression_effects(&field.value, effects, model, source);
                }
            }
            Statement::If(value) => {
                collect_expression_effects(&value.condition, effects, model, source);
                collect_block_effects(&value.then_block, effects, model, source);
                if let Some(block) = &value.else_block {
                    collect_block_effects(block, effects, model, source);
                }
            }
            Statement::Match(value) => {
                collect_expression_effects(&value.subject, effects, model, source);
                for arm in &value.arms {
                    collect_block_effects(&arm.body, effects, model, source);
                }
            }
            Statement::Assert(value) => {
                collect_expression_effects(&value.condition, effects, model, source)
            }
            Statement::AdvanceClock(value) => {
                collect_expression_effects(&value.duration, effects, model, source)
            }
            Statement::Unsupported(_) => {}
        }
    }
}

fn collect_expression_effects(
    expression: &Expression,
    effects: &mut OperationEffects,
    model: &mut PolicyModel,
    source: &str,
) {
    match expression {
        Expression::Create(value) => {
            let entity = joined(&value.target);
            effects
                .entities
                .entry(entity.clone())
                .or_default()
                .insert(PolicyEffect::Create);
            for field in &value.fields {
                if model
                    .bindings
                    .iter()
                    .any(|binding| binding.entity == entity && binding.field == field.name.text)
                    && !role_field_value_is_principal_derived(
                        &field.value,
                        &entity,
                        &field.name.text,
                    )
                {
                    push(
                        &mut model.diagnostics,
                        Diagnostic::error("POLICY_ROLE_FIELD_CREATE_FORBIDDEN").with_fact(
                            DiagnosticFact::Field(format!("{entity}.{}", field.name.text)),
                        ),
                        source,
                        field.range,
                    );
                }
                collect_expression_effects(&field.value, effects, model, source);
            }
        }
        Expression::Query(value) => {
            effects
                .entities
                .entry(joined(&value.target))
                .or_default()
                .insert(PolicyEffect::Read);
            if let Some(page) = &value.page {
                for predicate in &page.predicates {
                    collect_expression_effects(&predicate.value, effects, model, source);
                }
                collect_expression_effects(&page.after, effects, model, source);
                collect_expression_effects(&page.limit, effects, model, source);
            } else {
                collect_expression_effects(&value.value, effects, model, source);
            }
        }
        Expression::Update(value) => {
            let entity = joined(&value.target);
            effects
                .entities
                .entry(entity.clone())
                .or_default()
                .insert(PolicyEffect::Update);
            collect_expression_effects(&value.value, effects, model, source);
            for field in value.changes.iter().chain(
                value
                    .conditional_changes
                    .iter()
                    .map(|change| &change.change),
            ) {
                collect_expression_effects(&field.value, effects, model, source);
            }
        }
        Expression::Delete(value) => {
            effects
                .entities
                .entry(joined(&value.target))
                .or_default()
                .insert(PolicyEffect::Delete);
            collect_expression_effects(&value.value, effects, model, source);
        }
        Expression::Invocation(value) => {
            effects.calls.insert(joined(&value.callee));
            for argument in &value.arguments {
                collect_expression_effects(argument, effects, model, source);
            }
            for argument in &value.named_arguments {
                collect_expression_effects(&argument.value, effects, model, source);
            }
        }
        Expression::TestCall(value) => {
            effects.calls.insert(joined(&value.invocation.callee));
            for argument in &value.invocation.arguments {
                collect_expression_effects(argument, effects, model, source);
            }
            for argument in &value.invocation.named_arguments {
                collect_expression_effects(&argument.value, effects, model, source);
            }
        }
        Expression::Construction(value) => {
            for field in &value.fields {
                collect_expression_effects(&field.value, effects, model, source);
            }
        }
        Expression::Object(value) => {
            for field in &value.fields {
                collect_expression_effects(&field.value, effects, model, source);
            }
        }
        Expression::Attempt(value) => {
            collect_expression_effects(&value.value, effects, model, source)
        }
        Expression::OutcomeMatch(value) => {
            collect_expression_effects(&value.subject, effects, model, source);
            for arm in &value.arms {
                match &arm.body {
                    jadpo_syntax::OutcomeMatchArmBody::Value(value) => {
                        collect_expression_effects(value, effects, model, source)
                    }
                    jadpo_syntax::OutcomeMatchArmBody::Reject(value) => {
                        for field in &value.values {
                            collect_expression_effects(&field.value, effects, model, source);
                        }
                    }
                    jadpo_syntax::OutcomeMatchArmBody::Propagate(_) => {}
                }
            }
        }
        Expression::Unary(value) => {
            collect_expression_effects(&value.value, effects, model, source)
        }
        Expression::Binary(value) => {
            collect_expression_effects(&value.left, effects, model, source);
            collect_expression_effects(&value.right, effects, model, source);
        }
        Expression::Grouped(value) => {
            collect_expression_effects(&value.value, effects, model, source)
        }
        Expression::Literal(_) | Expression::Name(_) | Expression::Missing(_) => {}
    }
}

fn role_field_value_is_principal_derived(
    expression: &Expression,
    entity: &str,
    field: &str,
) -> bool {
    let is_principal_name = |expression: &Expression| {
        matches!(expression, Expression::Name(name)
            if name.path.first().is_some_and(|part| part.text == "principal"))
    };
    if is_principal_name(expression) {
        return true;
    }

    matches!(expression, Expression::Invocation(invocation)
        if invocation.callee.path.len() == 2
            && invocation.callee.path[0].text == entity
            && invocation.callee.path[1].text == field
            && invocation.arguments.len() == 1
            && invocation.named_arguments.is_empty()
            && is_principal_name(&invocation.arguments[0]))
}

fn resolve_call(call: &str, names: &BTreeSet<String>) -> String {
    if names.contains(call) {
        return call.to_owned();
    }
    let suffix = call.rsplit('.').next().unwrap_or(call);
    let mut candidates = names
        .iter()
        .filter(|name| name.ends_with(&format!(".{suffix}")));
    match (candidates.next(), candidates.next()) {
        (Some(value), None) => value.clone(),
        _ => call.to_owned(),
    }
}

fn route_method(method: jadpo_syntax::HttpMethod) -> &'static str {
    match method {
        jadpo_syntax::HttpMethod::Get => "GET",
        jadpo_syntax::HttpMethod::Post => "POST",
        jadpo_syntax::HttpMethod::Put => "PUT",
        jadpo_syntax::HttpMethod::Patch => "PATCH",
        jadpo_syntax::HttpMethod::Delete => "DELETE",
    }
}

fn check_rules(
    policy: &PolicyDeclaration,
    enums: &BTreeMap<String, BTreeSet<String>>,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<PolicyRuleContract> {
    policy
        .rules
        .iter()
        .map(|rule| {
            let subject = joined(&rule.subject);
            if !is_access_subject(&rule.subject) && !valid_role(&rule.subject, enums) {
                push(
                    diagnostics,
                    Diagnostic::error("POLICY_BINDING_INVALID")
                        .with_fact(DiagnosticFact::Name(subject.clone())),
                    source,
                    rule.subject.range,
                );
            }
            PolicyRuleContract {
                subject,
                effects: rule
                    .effects
                    .iter()
                    .map(|effect| effect.as_str().to_owned())
                    .collect(),
            }
        })
        .collect()
}

fn validate_rule_duplicates(
    policy: &PolicyDeclaration,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen = BTreeSet::new();
    for rule in &policy.rules {
        let subject = joined(&rule.subject);
        if !seen.insert(subject.clone()) {
            push(
                diagnostics,
                Diagnostic::error("POLICY_SUBJECT_DUPLICATE")
                    .with_fact(DiagnosticFact::Name(subject)),
                source,
                rule.range,
            );
        }
    }
}

fn valid_role(subject: &NameExpression, enums: &BTreeMap<String, BTreeSet<String>>) -> bool {
    subject.path.len() == 2
        && enums
            .get(&subject.path[0].text)
            .is_some_and(|variants| variants.contains(&subject.path[1].text))
}

fn is_access_subject(subject: &NameExpression) -> bool {
    subject.path.len() == 2
        && subject.path[0].text == "Access"
        && matches!(subject.path[1].text.as_str(), "public" | "authenticated")
}

fn field_by_name<'a>(
    record: &'a RecordDeclaration,
    name: &str,
) -> Option<&'a jadpo_syntax::FieldDeclaration> {
    record.fields.iter().find(|field| field.name.text == name)
}

fn referenced_entity(field: &jadpo_syntax::FieldDeclaration) -> Option<String> {
    let reference = field.reference.as_ref().map(|value| &value.target);
    let field_type = reference.unwrap_or(&field.field_type);
    (field_type.path.len() >= 2).then(|| field_type.path[0].text.clone())
}

fn type_name(reference: &jadpo_syntax::TypeReference) -> String {
    reference
        .path
        .iter()
        .map(|part| part.text.as_str())
        .collect::<Vec<_>>()
        .join(".")
}

fn joined(expression: &NameExpression) -> String {
    expression
        .path
        .iter()
        .map(|part| part.text.as_str())
        .collect::<Vec<_>>()
        .join(".")
}

fn push(
    diagnostics: &mut Vec<Diagnostic>,
    mut diagnostic: Diagnostic,
    source: &str,
    range: TextRange,
) {
    diagnostic.primary = Some(SourceSpan {
        source: source.to_owned(),
        start: range.start,
        end: range.end,
    });
    diagnostics.push(diagnostic);
}
