//! Checked source conditions and may-call links, never effective permission.
use super::{canonical_bytes, digest, AnalyzedProject, Declaration, NodeKind, Value};
use jadpo_semantic::NodeId;
use jadpo_syntax::{PersistenceModifier, RecordKind};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

fn id(domain: &str, tuple: Value) -> String {
    digest(canonical_bytes(&json!([domain, tuple])).as_bytes())
}

fn identity_fields(project: &AnalyzedProject) -> BTreeMap<String, String> {
    project
        .syntax
        .sources
        .iter()
        .flat_map(|source| &source.file.declarations)
        .filter_map(|declaration| {
            let Declaration::Record(record) = declaration else {
                return None;
            };
            if record.kind != RecordKind::Entity {
                return None;
            }
            let identity = record
                .dossier
                .as_ref()
                .map(|dossier| &dossier.identity.text)
                .or_else(|| {
                    record
                        .fields
                        .iter()
                        .find(|field| field.persistence.contains(&PersistenceModifier::Identity))
                        .map(|field| &field.name.text)
                })?;
            Some((record.name.text.clone(), identity.clone()))
        })
        .collect()
}

// A checked nominal reference can name a nonidentity field. Do not compose it
// with principal-side identity candidates merely because its entity root matches.
fn value_reference(
    project: &AnalyzedProject,
    identities: &BTreeMap<String, String>,
    entity: &str,
    field: &str,
    reference_entity: &str,
) -> Value {
    let expected = identities
        .get(reference_entity)
        .map(|identity| format!("{reference_entity}.{identity}"));
    for source in &project.syntax.sources {
        for declaration in &source.file.declarations {
            let Declaration::Record(record) = declaration else {
                continue;
            };
            if record.name.text != entity {
                continue;
            }
            let Some(declared) = record
                .fields
                .iter()
                .find(|candidate| candidate.name.text == field)
            else {
                continue;
            };
            let path = |reference: &jadpo_syntax::TypeReference| {
                reference
                    .path
                    .iter()
                    .map(|part| part.text.as_str())
                    .collect::<Vec<_>>()
                    .join(".")
            };
            let (reference, origin) = if let Some(reference) = &declared.reference {
                (Some(path(&reference.target)), "explicit_reference")
            } else if declared.field_type.path.len() >= 2 {
                (Some(path(&declared.field_type)), "nominal_field_type")
            } else if entity == reference_entity
                && identities
                    .get(entity)
                    .is_some_and(|identity| identity == field)
            {
                (Some(format!("{entity}.{field}")), "direct_self_identity")
            } else {
                (None, "no_checked_reference")
            };
            let composition = match (&reference, &expected) {
                (Some(reference), Some(expected)) if reference == expected => {
                    "checked_identity_reference"
                }
                (Some(_), Some(_)) => "not_identity_reference",
                _ => "unresolved",
            };
            return json!({"entity":entity,"field":field,
                "declared_type_tokens":super::contract_tokens(source,declared.field_type.range),
                "declared_type_path":path(&declared.field_type),
                "reference":reference,"reference_origin":origin,
                "expected_identity_reference":expected,"composition":composition});
        }
    }
    json!({"entity":entity,"field":field,"declared_type_tokens":null,"declared_type_path":null,
        "reference":null,"reference_origin":"checked_field_unavailable",
        "expected_identity_reference":expected,"composition":"unresolved"})
}

fn principal_candidates(
    entity: &str,
    identity: Option<&String>,
    facts: &BTreeMap<String, Value>,
) -> Vec<Value> {
    let mut candidates = BTreeMap::new();
    let validators = |variant: &str| {
        facts
            .iter()
            .filter(|(key, value)| {
                key.starts_with("auth_validator:") && value["principal_variant"] == variant
            })
            .map(|(key, value)| json!({"contract_id":key,"contract":value}))
            .collect::<Vec<_>>()
    };
    for (key, value) in facts {
        if key.starts_with("principal_variant:") {
            let Some(fields) = value["fields"].as_object() else {
                continue;
            };
            for (field, contract) in fields {
                if identity
                    .is_some_and(|identity| contract["type"] == format!("{entity}.{identity}"))
                {
                    let target = format!(
                        "{}.{}.{field}",
                        value["principal"].as_str().unwrap(),
                        value["variant"].as_str().unwrap()
                    );
                    candidates.insert(format!("nominal:{target}"), json!({
                        "kind":"nominal_identity_field","contract_id":key,"target":target,
                        "principal":value["principal"],"variant":value["variant"],"principal_kind":value["kind"],
                        "entity":entity,"identity_field":identity,"field_contract":contract,
                        "identity_composition":"explicit_nominal_identity_reference",
                        "validator_candidates":validators(value["variant"].as_str().unwrap())
                    }));
                }
            }
        } else if key.starts_with("auth_resolution:") {
            let Some(authority) = value["authority"].as_str() else {
                continue;
            };
            let Some((authority_entity, _)) = authority.rsplit_once('.') else {
                continue;
            };
            if authority_entity != entity {
                continue;
            }
            for (target, source) in value["mappings"].as_object().into_iter().flatten() {
                let maps_identity =
                    identity.is_some_and(|identity| source.as_str() == Some(identity.as_str()));
                candidates.insert(format!("resolution:{key}:{target}"), json!({
                    "kind":"authority_resolution","contract_id":key,"contract":value,
                    "entity":entity,"identity_field":identity,"source_field":source,"target":target,
                    "identity_composition":if maps_identity { "explicit_identity_mapping" } else { "not_present" },
                    "validator_candidates":validators(value["principal_variant"].as_str().unwrap())
                }));
            }
        }
    }
    candidates.into_values().collect()
}

fn source_candidates(
    project: &AnalyzedProject,
    facts: &BTreeMap<String, Value>,
    identities: &BTreeMap<String, String>,
    subject: &str,
    scope: Option<&str>,
) -> Vec<Value> {
    if matches!(subject, "Access.public" | "Access.authenticated") {
        return vec![json!({"kind":"builtin_subject","contract_id":subject,
            "principal_requirement":if subject == "Access.public" { "none_on_this_surface" } else { "any_valid_user_or_service_principal" },
            "route_authentication_override":false})];
    }
    let mut sources = BTreeMap::new();
    for binding in &project.policy.bindings {
        if binding.role != subject || Some(binding.scope.as_str()) != scope {
            continue;
        }
        let key = format!("role_binding:{}.{}", binding.entity, binding.field);
        let reference = value_reference(
            project,
            identities,
            &binding.entity,
            &binding.field,
            &binding.principal,
        );
        sources.insert(key.clone(),json!({"kind":"direct_role_binding","contract_id":key,"contract":facts[&key],"scope_identity_field":identities.get(&binding.entity),
            "source_value_reference":reference,"source_value_reference_composition":reference["composition"],
            "conditions":{"status":"required_not_proved","principal_equality":"validated_principal_identity_equals_binding_field","scope_equality":"referenced_scope_key_equals_protected_scope_key","current_row":"not_proved"},
            "principal_candidates":principal_candidates(&binding.principal,identities.get(&binding.principal),facts)}));
    }
    if let Some((role_type, variant)) = subject.rsplit_once('.') {
        for membership in &project.policy.memberships {
            if membership.role_type != role_type
                || !(membership.scope == "application" || Some(membership.scope.as_str()) == scope)
            {
                continue;
            }
            let key = format!("membership:{}", membership.entity);
            let reference = value_reference(
                project,
                identities,
                &membership.entity,
                &membership.member_field,
                &membership.principal,
            );
            let scope_reference = membership.scope_field.as_ref().map(|field| {
                value_reference(
                    project,
                    identities,
                    &membership.entity,
                    field,
                    &membership.scope,
                )
            });
            sources.insert(key.clone(),json!({"kind":"membership_binding","contract_id":key,"contract":facts[&key],
                "source_value_reference":reference,"source_value_reference_composition":reference["composition"],
                "scope_value_reference":scope_reference,"scope_reference_requirement":if membership.scope == "application" { "application_wide_no_value_reference" } else { "checked_resource_reference_required" },
                "conditions":{"status":"required_not_proved","principal_equality":"validated_principal_identity_equals_member_field","scope_equality":if membership.scope == "application" { "application_wide" } else { "membership_scope_key_equals_protected_scope_key" },"role_variant":variant,"current_row":"not_proved"},
                "principal_candidates":principal_candidates(&membership.principal,identities.get(&membership.principal),facts)}));
        }
    }
    sources.into_values().collect()
}

pub(super) fn derive(project: &AnalyzedProject, facts: &mut BTreeMap<String, Value>) -> Value {
    let identities = identity_fields(project);
    let mut operations = BTreeMap::new();
    // Original per-operation surfaces retain provenance. Route copies remain
    // separately identified summaries, not invented original enforcement sites.
    for operation in &project.policy.operations {
        let mut surfaces = BTreeMap::new();
        let inputs = operation
            .obligations
            .iter()
            .map(|obligation| {
                (
                    "operation_obligation",
                    &obligation.entity,
                    obligation.effect.as_str(),
                    None,
                    obligation.source,
                    &obligation.subjects,
                )
            })
            .chain(operation.field_reads.iter().map(|read| {
                (
                    "restricted_field_read",
                    &read.entity,
                    "read",
                    Some(read.field.as_str()),
                    read.source,
                    &read.subjects,
                )
            }));
        for (kind, entity, effect, field, provenance, subjects) in inputs {
            let subject_group = subjects
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let policy = project
                .policy
                .entities
                .iter()
                .find(|policy| &policy.entity == entity);
            let scope = if effect == "invoke" {
                Some("application")
            } else {
                policy.and_then(|policy| policy.scope.as_deref())
            };
            let scope_field = policy.and_then(|policy| policy.scope_field.as_deref());
            let surface_id = id(
                "actor_surface.v1",
                json!([
                    operation.operation,
                    kind,
                    entity,
                    effect,
                    field,
                    provenance,
                    subject_group
                ]),
            );
            let actors = subject_group.iter().map(|subject| {
                let sources = source_candidates(project,facts,&identities,subject,scope);
                let candidate_status = if sources.is_empty() { "no_checked_source_candidate" } else { "checked_source_conditions_only" };
                for source in &sources {
                    let link_id = id("actor_link.v1",json!([operation.operation,kind,entity,effect,field,provenance,subject_group,subject,source["contract_id"]]));
                    facts.insert(format!("actor_link:{link_id}"),json!({"operation":operation.operation,"surface_id":surface_id,"subject_group":subject_group,"subject":subject,"source":source,"scope":scope,"scope_field":scope_field}));
                }
                json!({"subject":subject,"candidate_status":candidate_status,"sources":sources})
            }).collect::<Vec<_>>();
            surfaces.insert(surface_id.clone(),json!({"id":surface_id,"operation":operation.operation,"kind":kind,"entity":entity,"effect":effect,"field":field,
                "provenance":provenance,"surface_origin":if provenance == "transitive_operation" { "copied_route_aggregate" } else { "checked_operation_surface" },
                "scope":scope,"scope_field":scope_field,"target_identity_field":identities.get(entity),
                "subject_group":subject_group,"subject_composition":"alternatives_or","actors":actors}));
        }
        let contract = json!({"operation":operation.operation,"surface_composition":"requirements_and","surfaces":surfaces.into_values().collect::<Vec<_>>()});
        facts.insert(
            format!("operation_actor_sources:{}", operation.operation),
            contract.clone(),
        );
        operations.insert(operation.operation.clone(), contract);
    }
    let graph = &project.semantics;
    let mut adjacency = BTreeMap::<NodeId, Vec<NodeId>>::new();
    for edge in &graph.calls {
        adjacency.entry(edge.caller).or_default().push(edge.callee);
    }
    for children in adjacency.values_mut() {
        children.sort_by_key(|id| &graph.nodes[id.0 as usize].name);
        children.dedup();
    }
    let mut routes = BTreeMap::new();
    let authentication_candidates = facts
        .iter()
        .filter(|(key, _)| {
            key.starts_with("application_auth:")
                || key.starts_with("principal_variant:")
                || key.starts_with("auth_")
        })
        .map(|(key, value)| json!({"contract_id":key,"contract":value}))
        .collect::<Vec<_>>();
    for route in graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Route)
    {
        let mut queue = VecDeque::from([(route.id, vec![route.name.clone()])]);
        let mut visited = BTreeSet::new();
        let mut reached = BTreeMap::new();
        while let Some((node_id, witness)) = queue.pop_front() {
            if !visited.insert(node_id) {
                continue;
            }
            let node = &graph.nodes[node_id.0 as usize];
            let operation = if node.kind == NodeKind::Route {
                node.name
                    .split_once(' ')
                    .map(|(method, path)| format!("route:{method}:{path}"))
            } else if matches!(
                node.kind,
                NodeKind::Action | NodeKind::Query | NodeKind::Function
            ) {
                Some(node.name.clone())
            } else {
                None
            };
            if let Some((name, contract)) = operation
                .as_ref()
                .and_then(|name| operations.get_key_value(name))
            {
                reached.insert(
                    name.clone(),
                    json!({"operation":name,"path":witness,"contract":contract}),
                );
            }
            for child in adjacency.get(&node_id).into_iter().flatten() {
                let mut path = witness.clone();
                path.push(graph.nodes[child.0 as usize].name.clone());
                queue.push_back((*child, path));
            }
        }
        let route_is_public = project.syntax.sources.iter().flat_map(|source| &source.file.declarations).any(|declaration|
            matches!(declaration,Declaration::Route(declaration) if declaration.public && format!("{} {}",super::method_name(declaration.method),declaration.path) == route.name));
        let contract = json!({"route":route.name,"status":"compiler_checked_source_conditions_and_may_call",
            "operation_composition":"requirements_and","operations":reached.into_values().collect::<Vec<_>>(),
            "entry_authentication_candidates":if route_is_public { vec![] } else { authentication_candidates.clone() },
            "route_validator_selection":"not_resolved","live_authority":"not_proved","feasibility":"not_analyzed"});
        facts.insert(
            format!("route_actor_sources:{}", route.name),
            contract.clone(),
        );
        routes.insert(route.name.clone(), contract);
    }
    json!({"status":"compiler_checked_source_conditions_and_may_call","operations":operations,"routes":routes,
        "live_authority":"not_proved","feasibility":"not_analyzed"})
}
