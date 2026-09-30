use crate::{checked_source_revision, AnalyzedProject};
use jadpo_diagnostics::{catalogue_manifest_json, catalogue_reference_markdown, Diagnostic};
use jadpo_semantic::checked_manifest_json;
use jadpo_syntax::{
    CallableKind, ConfigDeclaration, Constraint, ConstraintKind, Declaration, EnumDeclaration,
    FailureDeclaration, FieldDeclaration, HttpMethod, LiteralKind, PersistenceModifier,
    RecordDeclaration, RecordKind, TypeDeclaration, TypeReference,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static OUTPUT_REVISION: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedArtifact {
    pub relative_path: &'static str,
    pub contents: String,
}

pub fn derive_artifacts(project_path: &Path, project: &AnalyzedProject) -> Vec<GeneratedArtifact> {
    let model = ArtifactModel::new(project);
    let metadata = normalized_manifest(project_path, project);
    let mut outputs = vec![
        artifact("app.meta.json", metadata),
        artifact("inventory/routes.json", model.routes_json()),
        artifact("inventory/callables.json", model.callables_json()),
        artifact("audit/failures.json", model.failure_audit_json()),
        artifact("audit/entities.json", model.entity_audit_json()),
        artifact("audit/transactions.json", model.transaction_audit_json()),
        artifact("audit/configuration.json", model.configuration_audit_json()),
        artifact("audit/policy.json", model.policy_audit_json()),
        artifact("validators/plan.json", model.validator_plan_json()),
        artifact(
            "compatibility/public-failure-codes.json",
            model.public_failure_compatibility_json(),
        ),
        artifact("openapi/openapi.json", model.openapi_json()),
        artifact("diagnostics/catalogue.json", catalogue_manifest_json()),
        artifact("diagnostics/reference.md", catalogue_reference_markdown()),
    ];
    if project.syntax.sources.iter().any(|s| {
        s.file
            .declarations
            .iter()
            .any(|d| matches!(d, Declaration::AuthenticationStrategy(_)))
    }) {
        outputs.push(artifact(
            "audit/authentication.json",
            model.authentication_audit_json(project_path),
        ));
    }
    outputs
}

fn normalized_manifest(project_path: &Path, project: &AnalyzedProject) -> String {
    let project_root = if project_path.is_dir() {
        project_path
    } else {
        project_path.parent().unwrap_or_else(|| Path::new("."))
    };
    let mut graph = project.semantics.clone();
    for node in &mut graph.nodes {
        node.source = normalized_source(project_root, &node.source);
    }
    let mut typing = project.typing.clone();
    for expression in &mut typing.expressions {
        expression.source = normalized_source(project_root, &expression.source);
    }
    let graph_json = checked_manifest_json(&graph, &typing, &project.failures);
    let operations = project
        .syntax
        .sources
        .iter()
        .flat_map(|source| {
            source.file.declarations.iter().filter_map(|declaration| {
                let Declaration::Route(route) = declaration else {
                    return None;
                };
                Some(format!(
                    "{{\"id\":{},\"source\":{},\"range\":{{\"start\":{},\"end\":{}}},\"route\":{}}}",
                    json_string(&format!("route:{}:{}", method_name(route.method), route.path)),
                    json_string(&normalized_source(project_root, &source.source_name)),
                    route.range.start,
                    route.range.end,
                    json_string(&format!("{} {}", method_name(route.method), route.path)),
                ))
            })
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema_version\":2,\"source_revision\":{},\"operations\":[{operations}],\"semantic_graph\":{graph_json}}}",
        json_string(&checked_source_revision(project_path, project))
    )
}

fn normalized_source(project_root: &Path, source: &str) -> String {
    if source.starts_with('<') {
        return source.to_owned();
    }
    Path::new(source)
        .strip_prefix(project_root)
        .unwrap_or_else(|_| Path::new(source))
        .to_string_lossy()
        .replace('\\', "/")
}

pub fn write_artifacts(
    project_path: &Path,
    artifacts: &[GeneratedArtifact],
) -> Result<PathBuf, Diagnostic> {
    let project_root = if project_path.is_dir() {
        project_path
    } else {
        project_path.parent().unwrap_or_else(|| Path::new("."))
    };
    let output_root = project_root.join("build");
    let revision = OUTPUT_REVISION.fetch_add(1, Ordering::Relaxed);
    let suffix = format!("{}.{}", std::process::id(), revision);
    let staging_root = project_root.join(format!(".jadpo-build-stage.{suffix}"));
    let backup_root = project_root.join(format!(".jadpo-build-backup.{suffix}"));

    fs::create_dir(&staging_root)
        .map_err(|_error| Diagnostic::error("JADPO_ARTIFACT_STAGE_FAILED"))?;

    for artifact in artifacts {
        let destination = staging_root.join(artifact.relative_path);
        if let Some(parent) = destination.parent() {
            if let Err(_error) = fs::create_dir_all(parent) {
                let _ = fs::remove_dir_all(&staging_root);
                return Err(Diagnostic::error("JADPO_ARTIFACT_WRITE_FAILED"));
            }
        }
        if let Err(_error) = fs::write(&destination, &artifact.contents) {
            let _ = fs::remove_dir_all(&staging_root);
            return Err(Diagnostic::error("JADPO_ARTIFACT_WRITE_FAILED"));
        }
    }

    let had_previous = output_root.exists();
    if had_previous {
        if let Err(_error) = fs::rename(&output_root, &backup_root) {
            let _ = fs::remove_dir_all(&staging_root);
            return Err(Diagnostic::error("JADPO_ARTIFACT_PROMOTE_FAILED"));
        }
    }

    if let Err(_error) = fs::rename(&staging_root, &output_root) {
        if had_previous {
            let _ = fs::rename(&backup_root, &output_root);
        }
        let _ = fs::remove_dir_all(&staging_root);
        return Err(Diagnostic::error("JADPO_ARTIFACT_PROMOTE_FAILED"));
    }

    if had_previous {
        fs::remove_dir_all(&backup_root)
            .map_err(|_error| Diagnostic::error("JADPO_ARTIFACT_CLEANUP_FAILED"))?;
    }

    Ok(output_root)
}

fn artifact(relative_path: &'static str, mut contents: String) -> GeneratedArtifact {
    contents.push('\n');
    GeneratedArtifact {
        relative_path,
        contents,
    }
}

struct ArtifactModel<'project> {
    project: &'project AnalyzedProject,
    types: BTreeMap<String, &'project TypeDeclaration>,
    enums: BTreeMap<String, &'project EnumDeclaration>,
    records: BTreeMap<String, &'project RecordDeclaration>,
    fields: BTreeMap<String, &'project FieldDeclaration>,
    failure_declarations: BTreeMap<String, &'project FailureDeclaration>,
    routes: Vec<RouteModel>,
    callables: Vec<CallableModel>,
    configurations: Vec<&'project ConfigDeclaration>,
}

#[derive(Clone)]
struct RouteModel {
    key: String,
    method: &'static str,
    path: String,
    public: bool,
    fresh_authority: bool,
    path_fields: Vec<(String, String)>,
    input: Option<String>,
    output: Option<String>,
    callable: Option<String>,
    behavior: &'static str,
}

#[derive(Clone)]
struct CallableModel {
    name: String,
    kind: &'static str,
    parameters: Vec<(String, String)>,
    output: String,
    failures: Vec<String>,
    may_suspend: bool,
}

impl<'project> ArtifactModel<'project> {
    fn new(project: &'project AnalyzedProject) -> Self {
        let mut types = BTreeMap::new();
        let mut enums = BTreeMap::new();
        let mut records = BTreeMap::new();
        let mut fields = BTreeMap::new();
        let mut failure_declarations = BTreeMap::new();
        let mut routes = Vec::new();
        let mut callables = Vec::new();
        let mut configurations = Vec::new();

        for source in &project.syntax.sources {
            for declaration in &source.file.declarations {
                match declaration {
                    Declaration::Config(declaration) => configurations.push(declaration),
                    Declaration::Type(declaration) => {
                        types.insert(declaration.name.text.clone(), declaration);
                    }
                    Declaration::Enum(declaration) => {
                        for variant in &declaration.variants {
                            for field in &variant.fields {
                                fields.insert(
                                    format!(
                                        "{}.{}.{}",
                                        declaration.name.text, variant.name.text, field.name.text
                                    ),
                                    field,
                                );
                            }
                        }
                        enums.insert(declaration.name.text.clone(), declaration);
                    }
                    Declaration::Record(declaration) => {
                        for field in &declaration.fields {
                            fields.insert(
                                format!("{}.{}", declaration.name.text, field.name.text),
                                field,
                            );
                        }
                        records.insert(declaration.name.text.clone(), declaration);
                    }
                    Declaration::Callable(declaration) => {
                        let mut failures = declaration
                            .failures
                            .iter()
                            .map(|failure| failure.text.clone())
                            .collect::<Vec<_>>();
                        failures.sort();
                        callables.push(CallableModel {
                            name: declaration.name.text.clone(),
                            kind: match declaration.kind {
                                CallableKind::Function => "function",
                                CallableKind::Action => "action",
                                CallableKind::Query => "query",
                            },
                            parameters: declaration
                                .parameters
                                .iter()
                                .map(|parameter| {
                                    (
                                        parameter.name.text.clone(),
                                        type_name(&parameter.parameter_type),
                                    )
                                })
                                .collect(),
                            output: type_name(&declaration.return_type),
                            failures,
                            may_suspend: project
                                .failures
                                .callables
                                .iter()
                                .find(|callable| callable.callable == declaration.name.text)
                                .is_some_and(|callable| callable.may_suspend),
                        });
                    }
                    Declaration::Route(declaration) => {
                        let method = method_name(declaration.method);
                        routes.push(RouteModel {
                            key: format!("{method} {}", declaration.path),
                            method,
                            path: declaration.path.clone(),
                            public: declaration.public,
                            fresh_authority: declaration.fresh_authority,
                            path_fields: declaration
                                .path_fields
                                .iter()
                                .map(|field| {
                                    (field.name.text.clone(), type_name(&field.field_type))
                                })
                                .collect(),
                            input: declaration.input.as_ref().map(type_name),
                            output: declaration.output.as_ref().map(type_name),
                            callable: declaration.run.as_ref().map(|run| {
                                run.callee
                                    .path
                                    .iter()
                                    .map(|part| part.text.as_str())
                                    .collect::<Vec<_>>()
                                    .join(".")
                            }),
                            behavior: if declaration.inline_action.is_some() {
                                "inline_action"
                            } else {
                                "run"
                            },
                        });
                    }
                    Declaration::Failure(declaration) => {
                        failure_declarations.insert(declaration.name.text.clone(), declaration);
                        for (scope, declarations) in [
                            ("public", &declaration.public_fields),
                            ("internal", &declaration.internal_fields),
                        ] {
                            for field in declarations {
                                fields.insert(
                                    format!(
                                        "{}.{}.{}",
                                        declaration.name.text, scope, field.name.text
                                    ),
                                    field,
                                );
                            }
                        }
                    }
                    Declaration::Application(_)
                    | Declaration::Locales(_)
                    | Declaration::AuthenticationStrategy(_)
                    | Declaration::Principal(_)
                    | Declaration::Fixture(_)
                    | Declaration::Test(_) => {}
                }
            }
        }
        routes.sort_by(|left, right| left.key.cmp(&right.key));
        callables.sort_by(|left, right| left.name.cmp(&right.name));

        Self {
            project,
            types,
            enums,
            records,
            fields,
            failure_declarations,
            routes,
            callables,
            configurations,
        }
    }

    fn configuration_audit_json(&self) -> String {
        let fields = self
            .configurations
            .iter()
            .flat_map(|configuration| {
                configuration.fields.iter().map(|field| {
                    let binding = field
                        .binding
                        .as_ref()
                        .map(|binding| json_string(unquote(&binding.text)))
                        .unwrap_or_else(|| "null".to_owned());
                    format!(
                        "{{\"configuration\":{},\"name\":{},\"type\":{},\"binding\":{binding},\"secret\":{},\"required\":{},\"lifecycle\":\"startup\",\"source_default\":{}}}",
                        json_string(&configuration.name.text),
                        json_string(&field.name.text),
                        json_string(&type_name(&field.field_type)),
                        field.secret,
                        field.default.is_none(),
                        field.default.as_ref().map_or("false", |_| "true"),
                    )
                })
            })
            .collect::<Vec<_>>()
            .join(",");
        format!("{{\"schema_version\":1,\"fields\":[{fields}]}}")
    }

    fn policy_audit_json(&self) -> String {
        let bindings = self.project.policy.bindings.iter().map(|binding| {
            format!(
                "{{\"kind\":\"direct\",\"entity\":{},\"field\":{},\"role\":{},\"scope\":{},\"principal\":{},\"authoritative\":true}}",
                json_string(&binding.entity),
                json_string(&binding.field),
                json_string(&binding.role),
                json_string(&binding.scope),
                json_string(&binding.principal),
            )
        });
        let memberships = self.project.policy.memberships.iter().map(|membership| {
            format!(
                "{{\"kind\":\"membership\",\"entity\":{},\"scope_field\":{},\"scope\":{},\"member_field\":{},\"principal\":{},\"role_field\":{},\"role_type\":{},\"authoritative\":true}}",
                json_string(&membership.entity),
                membership
                    .scope_field
                    .as_ref()
                    .map_or_else(|| "null".to_owned(), |field| json_string(field)),
                json_string(&membership.scope),
                json_string(&membership.member_field),
                json_string(&membership.principal),
                json_string(&membership.role_field),
                json_string(&membership.role_type),
            )
        });
        let entities = self.project.policy.entities.iter().map(|entity| {
            let rules = entity.rules.iter().map(|rule| {
                format!(
                    "{{\"subject\":{},\"effects\":{}}}",
                    json_string(&rule.subject),
                    json_string_array(rule.effects.iter().map(String::as_str)),
                )
            });
            let fields = entity.fields.iter().map(|field| {
                let rules = field.rules.iter().map(|rule| {
                    format!(
                        "{{\"subject\":{},\"effects\":{}}}",
                        json_string(&rule.subject),
                        json_string_array(rule.effects.iter().map(String::as_str)),
                    )
                });
                format!(
                    "{{\"field\":{},\"mode\":\"narrowing\",\"rules\":{}}}",
                    json_string(&field.field),
                    json_array(rules),
                )
            });
            let mut effect_first = BTreeMap::<&str, Vec<&str>>::new();
            for rule in &entity.rules {
                for effect in &rule.effects {
                    effect_first
                        .entry(effect)
                        .or_default()
                        .push(&rule.subject);
                }
            }
            let effect_first = effect_first.into_iter().map(|(effect, subjects)| {
                format!(
                    "{{\"effect\":{},\"subjects\":{}}}",
                    json_string(effect),
                    json_string_array(subjects),
                )
            });
            format!(
                "{{\"entity\":{},\"default\":\"deny\",\"scope\":{},\"scope_field\":{},\"role_first\":{},\"effect_first\":{},\"fields\":{}}}",
                json_string(&entity.entity),
                json_optional_string(entity.scope.as_deref()),
                json_optional_string(entity.scope_field.as_deref()),
                json_array(rules),
                json_array(effect_first),
                json_array(fields),
            )
        });
        let operations = self.project.policy.operations.iter().map(|operation| {
            let obligations = operation.obligations.iter().map(|obligation| {
                format!(
                    "{{\"entity\":{},\"effect\":{},\"subjects\":{},\"policy_source\":{},\"judgement\":{}}}",
                    json_string(&obligation.entity),
                    json_string(&obligation.effect),
                    json_string_array(obligation.subjects.iter().map(String::as_str)),
                    json_string(obligation.source),
                    json_string(if obligation.subjects.is_empty() { "rejected" } else { "proved" }),
                )
            });
            let field_reads = operation.field_reads.iter().map(|read| {
                format!(
                    "{{\"entity\":{},\"field\":{},\"subjects\":{},\"source\":{},\"judgement\":\"proved\"}}",
                    json_string(&read.entity),
                    json_string(&read.field),
                    json_string_array(read.subjects.iter().map(String::as_str)),
                    json_string(read.source),
                )
            });
            format!(
                "{{\"operation\":{},\"obligations\":{},\"restricted_field_reads\":{}}}",
                json_string(&operation.operation),
                json_array(obligations),
                json_array(field_reads),
            )
        });
        format!(
            "{{\"schema_version\":1,\"active\":{},\"bindings\":{},\"memberships\":{},\"entities\":{},\"operations\":{}}}",
            self.project.policy.active,
            json_array(bindings),
            json_array(memberships),
            json_array(entities),
            json_array(operations),
        )
    }

    fn routes_json(&self) -> String {
        let routes = self
            .routes
            .iter()
            .map(|route| {
                let failures = self.route_failures(&route.key);
                let path_fields = route.path_fields.iter().map(|(name, field_type)| {
                    format!(
                        "{{\"name\":{},\"type\":{}}}",
                        json_string(name),
                        json_string(field_type)
                    )
                });
                format!(
                    "{{\"route\":{},\"method\":{},\"path\":{},\"auth\":{},\"path_fields\":{},\"input\":{},\"output\":{},\"behavior\":{},\"callable\":{},\"failures\":{}}}",
                    json_string(&route.key),
                    json_string(route.method),
                    json_string(&route.path),
                    json_string(if route.public {
                        "none"
                    } else if route.fresh_authority {
                        "fresh_authority"
                    } else {
                        "authenticated_default"
                    }),
                    json_array(path_fields),
                    json_optional_string(route.input.as_deref()),
                    json_optional_string(route.output.as_deref()),
                    json_string(route.behavior),
                    json_optional_string(route.callable.as_deref()),
                    json_array(failures.into_iter().map(|failure| {
                        format!(
                            "{{\"name\":{},\"code\":{},\"http_status\":{},\"derived\":true}}",
                            json_string(failure.name),
                            json_string(failure.code),
                            failure.http_status
                        )
                    }))
                )
            });
        format!("{{\"schema_version\":1,\"routes\":{}}}", json_array(routes))
    }

    fn callables_json(&self) -> String {
        let callables = self.callables.iter().map(|callable| {
            let parameters = callable.parameters.iter().map(|(name, parameter_type)| {
                format!(
                    "{{\"name\":{},\"type\":{}}}",
                    json_string(name),
                    json_string(parameter_type)
                )
            });
            format!(
                "{{\"name\":{},\"kind\":{},\"parameters\":{},\"output\":{},\"failures\":{},\"execution\":{{\"may_suspend\":{},\"completion\":\"before_caller_continues\"}}}}",
                json_string(&callable.name),
                json_string(callable.kind),
                json_array(parameters),
                json_string(&callable.output),
                json_string_array(callable.failures.iter().map(String::as_str)),
                callable.may_suspend
            )
        });
        format!(
            "{{\"schema_version\":2,\"callables\":{}}}",
            json_array(callables)
        )
    }

    fn failure_audit_json(&self) -> String {
        let contracts = self.project.failures.contracts.iter().map(|contract| {
            let routes = self
                .project
                .failures
                .routes
                .iter()
                .filter(|route| route.failure == contract.name)
                .map(|route| route.route.as_str());
            format!(
                "{{\"failure\":{},\"code\":{},\"kind\":{},\"http_status\":{},\"message\":{},\"disclosure\":{{\"public_fields\":{},\"internal_fields\":{},\"internal_to_client\":false}},\"routes\":{}}}",
                json_string(&contract.name),
                json_string(&contract.code),
                json_string(&contract.kind),
                contract.http_status,
                json_optional_string(contract.message.as_deref()),
                json_string_array(contract.public_fields.iter().map(String::as_str)),
                json_string_array(contract.internal_fields.iter().map(String::as_str)),
                json_string_array(routes)
            )
        });
        format!(
            "{{\"schema_version\":1,\"policy\":{{\"closed_public_payloads\":true,\"internal_context_disclosed\":false}},\"failures\":{}}}",
            json_array(contracts)
        )
    }

    fn entity_audit_json(&self) -> String {
        let entities = self.project.entity_model.entities.iter().map(|entity| {
            let representations = entity.representations.iter().map(|representation| {
                format!(
                    "{{\"kind\":{},\"name\":{},\"store\":{},\"from_authority\":{},\"strategy\":{},\"required_delivery\":\"durable\",\"change_record\":\"same_local_commit\",\"ordering\":\"per_entity_revision\",\"change_id\":\"unique\",\"delivery_runtime\":\"adapter_pending\",\"replay\":false,\"rebuild\":false,\"watermark\":false,\"reconciliation\":false}}",
                    json_string(representation.kind),
                    json_string(&representation.name),
                    json_string(&representation.store),
                    json_string(&representation.authority),
                    json_optional_string(representation.strategy.as_deref())
                )
            });
            format!(
                "{{\"name\":{},\"identity\":{},\"persistent\":{},\"authority_store\":{},\"reference\":{},\"operations\":{},\"representations\":{}}}",
                json_string(&entity.name),
                json_string(&entity.identity),
                entity.persistent,
                json_optional_string(entity.authority_store.as_deref()),
                json_string(&format!("{}.Ref", entity.name)),
                json_string_array(entity.operations.iter().map(String::as_str)),
                json_array(representations)
            )
        });
        let queries = self.project.entity_model.queries.iter().map(|query| {
            format!(
                "{{\"name\":{},\"owner\":{},\"read_only\":true,\"freshness\":{},\"plan\":{},\"reads\":{},\"predicate_fields\":{},\"policy_source\":\"authority\"}}",
                json_string(&query.name),
                json_optional_string(query.owner.as_deref()),
                json_string(query.freshness),
                json_string(query.plan),
                json_string_array(query.reads.iter().map(String::as_str)),
                json_string_array(query.predicate_fields.iter().map(String::as_str))
            )
        });
        format!(
            "{{\"schema_version\":1,\"model\":\"DATA-007/CONSISTENCY-001\",\"entities\":{},\"queries\":{}}}",
            json_array(entities),
            json_array(queries)
        )
    }

    fn transaction_audit_json(&self) -> String {
        let actions = self.project.entity_model.transactions.iter().map(|transaction| {
            format!(
                "{{\"action\":{},\"mutation_owners\":{},\"disposition\":{},\"domain\":{},\"nested_actions\":{},\"handled_failure\":{},\"commit\":\"outer_success\",\"rollback\":\"propagated_failure\",\"postgres\":{{\"isolation\":{},\"concurrency\":{},\"deadlock_order\":\"stable_entity_identity\"}},\"sqlite\":{{\"isolation\":{},\"concurrency\":\"single_immediate_writer\"}},\"retry\":{}}}",
                json_string(&transaction.action),
                json_string_array(transaction.owners.iter().map(String::as_str)),
                json_string(transaction.disposition),
                json_optional_string(transaction.domain.as_deref()),
                json_string(transaction.nested),
                json_string(transaction.handled_failure),
                json_string(transaction.postgres_isolation),
                json_string(transaction.postgres_concurrency),
                json_string(transaction.sqlite_isolation),
                json_string(transaction.retry)
            )
        });
        format!(
            "{{\"schema_version\":1,\"model\":\"TX-001\",\"actions\":{}}}",
            json_array(actions)
        )
    }

    fn validator_plan_json(&self) -> String {
        let named_types = self.types.iter().map(|(name, declaration)| {
            format!(
                "{{\"name\":{},\"representation\":{},\"constraints\":{}}}",
                json_string(name),
                json_string(&type_name(&declaration.parent)),
                constraints_json(&declaration.constraints)
            )
        });
        let records = self.records.iter().map(|(name, declaration)| {
            let fields = declaration
                .fields
                .iter()
                .map(|field| field_plan_json(field, self.reference_is_nullable(&field.field_type)));
            format!(
                "{{\"name\":{},\"kind\":{},\"persistent\":{},\"closed_shape\":true,\"fields\":{}}}",
                json_string(name),
                json_string(record_kind(declaration.kind)),
                declaration.is_persistent_entity(),
                json_array(fields)
            )
        });
        let enums = self.enums.iter().map(|(name, declaration)| {
            let payloads = declaration.variants.iter().map(|variant| {
                format!(
                    "{{\"name\":{},\"fields\":{}}}",
                    json_string(&variant.name.text),
                    json_array(variant.fields.iter().map(|field| {
                        field_plan_json(field, self.reference_is_nullable(&field.field_type))
                    }))
                )
            });
            format!(
                "{{\"name\":{},\"tagged\":{},\"variants\":{},\"payloads\":{}}}",
                json_string(name),
                declaration
                    .variants
                    .iter()
                    .any(|variant| !variant.fields.is_empty()),
                json_string_array(
                    declaration
                        .variants
                        .iter()
                        .map(|variant| variant.name.text.as_str())
                ),
                json_array(payloads)
            )
        });
        let boundaries = self.routes.iter().map(|route| {
            format!(
                "{{\"route\":{},\"input\":{},\"output\":{},\"reject_unknown_input_fields\":true,\"validate_output\":true}}",
                json_string(&route.key),
                json_optional_string(route.input.as_deref()),
                json_optional_string(route.output.as_deref())
            )
        });
        format!(
            "{{\"schema_version\":1,\"named_types\":{},\"enums\":{},\"records\":{},\"boundaries\":{}}}",
            json_array(named_types),
            json_array(enums),
            json_array(records),
            json_array(boundaries)
        )
    }

    fn public_failure_compatibility_json(&self) -> String {
        let contracts = self.project.failures.contracts.iter().map(|contract| {
            format!(
                "{{\"code\":{},\"failure\":{},\"http_status\":{},\"message\":{},\"public_fields\":{}}}",
                json_string(&contract.code),
                json_string(&contract.name),
                contract.http_status,
                json_optional_string(contract.message.as_deref()),
                json_string_array(contract.public_fields.iter().map(String::as_str))
            )
        });
        format!(
            "{{\"schema_version\":1,\"baseline\":null,\"status\":\"baseline_not_configured\",\"breaking_change_keys\":[\"code\",\"http_status\",\"message\",\"public_fields\"],\"current\":{}}}",
            json_array(contracts)
        )
    }

    fn authentication_strategies(&self) -> Vec<&jadpo_syntax::AuthenticationStrategyDeclaration> {
        self.project
            .syntax
            .sources
            .iter()
            .flat_map(|s| &s.file.declarations)
            .filter_map(|d| match d {
                Declaration::AuthenticationStrategy(a) => Some(a),
                _ => None,
            })
            .collect()
    }

    fn authentication_audit_json(&self, project_path: &Path) -> String {
        let supported =
            crate::target::first_party_authentication_supported(project_path, self.project);
        let jwt = crate::target::jwt_authentication_supported(project_path, self.project);
        let dependencies = if jwt {
            format!("[{}]", include_str!("runtime/jwt/dependency.json"))
        } else {
            "[]".to_owned()
        };
        let unsupported = if supported {
            "[\"profile_bearing_principals\",\"public_login_endpoints\"]"
        } else {
            "[\"configured_runtime_pending\",\"profile_bearing_principals\",\"public_login_endpoints\"]"
        };
        let jwt_authority = if jwt { "\"verified_subject_resolves_once_on_every_request\"" } else { "null" };
        let strategies = self.authentication_strategies().into_iter().map(|strategy| {
            let (transport, location) = match &strategy.transport.location {
                jadpo_syntax::CredentialLocation::Cookie(cookie) => ("cookie", cookie.text.trim_matches('"')),
                jadpo_syntax::CredentialLocation::Bearer(_) => ("bearer", "authorization"),
            };
            let validators = strategy.validators.iter().map(|v| format!("{{\"name\":{},\"mode\":{},\"principal\":{},\"settings\":{}}}", json_string(&v.name.text), json_string(&v.mode.text), json_string(&v.principal.text), json_array(v.settings.iter().map(|s| json_string(&s.name.text)))));
            let resolutions = strategy.resolutions.iter().map(|r| format!("{{\"authority\":{},\"inactive_failure\":{}}}", json_string(&r.authority.path.iter().map(|n| n.text.as_str()).collect::<Vec<_>>().join(".")), json_string(&r.inactive.text)));
            format!("{{\"name\":{},\"transport\":{},\"location\":{},\"validators\":{},\"resolutions\":{},\"source_range\":{{\"start\":{},\"end\":{}}}}}", json_string(&strategy.name.text), json_string(transport), json_string(location), json_array(validators), json_array(resolutions), strategy.range.start, strategy.range.end)
        });
        let revocation = self
            .project
            .syntax
            .sources
            .iter()
            .flat_map(|s| &s.file.declarations)
            .find_map(|d| match d {
                Declaration::Application(a) => Some(format!(
                    "{{\"mode\":{},\"maximum_delay\":{}}}",
                    json_string(
                        if a.authentication.revocation.mode
                            == jadpo_syntax::RevocationMode::Immediate
                        {
                            "immediate"
                        } else {
                            "bounded"
                        }
                    ),
                    json_optional_string(
                        a.authentication
                            .revocation
                            .maximum_delay
                            .as_ref()
                            .map(|d| d.text.as_str())
                    )
                )),
                _ => None,
            })
            .unwrap_or("null".to_owned());
        format!("{{\"schema_version\":1,\"runtime\":{},\"revocation\":{},\"strategies\":{},\"routes\":{},\"credential_selection\":\"exactly_one\",\"verification\":\"runtime_validation\",\"package_dependencies\":{dependencies},\"csrf\":\"cookie_mutations_require_configured_origin_and_session_bound_header\",\"unsupported\":{unsupported},\"jwt_authority\":{jwt_authority},\"query_accounting\":\"principal_lookup_separate_from_credential_checks\",\"assurance\":\"exploratory; not an independent security review\"}}", json_string(if supported { "first_party" } else { "adapter_pending" }), revocation, json_array(strategies), self.routes_json())
    }

    fn openapi_json(&self) -> String {
        let strategies = self.authentication_strategies();
        let security_schemes = strategies
            .iter()
            .map(|strategy| {
                let scheme = match &strategy.transport.location {
                    jadpo_syntax::CredentialLocation::Cookie(cookie) => format!(
                        "{{\"type\":\"apiKey\",\"in\":\"cookie\",\"name\":{}}}",
                        json_string(cookie.text.trim_matches('"'))
                    ),
                    jadpo_syntax::CredentialLocation::Bearer(_) => {
                        "{\"type\":\"http\",\"scheme\":\"bearer\"}".to_owned()
                    }
                };
                format!("{}:{}", json_string(&strategy.name.text), scheme)
            })
            .collect::<Vec<_>>()
            .join(",");
        let mut paths = BTreeMap::<String, Vec<String>>::new();
        for route in &self.routes {
            let mut responses = Vec::new();
            if let Some(output) = &route.output {
                responses.push(format!(
                    "\"200\":{{\"description\":\"Success\",\"content\":{{\"application/json\":{{\"schema\":{}}}}}}}",
                    self.openapi_type_schema(output)
                ));
            }
            let mut failure_groups = BTreeMap::<u16, Vec<&str>>::new();
            for failure in self.route_failures(&route.key) {
                failure_groups
                    .entry(failure.http_status)
                    .or_default()
                    .push(failure.name);
            }
            for (status, names) in failure_groups {
                let references = names
                    .iter()
                    .map(|name| {
                        format!(
                            "{{\"$ref\":{}}}",
                            json_string(&format!("#/components/schemas/{name}"))
                        )
                    })
                    .collect::<Vec<_>>();
                let failure_schema = if references.len() == 1 {
                    references[0].clone()
                } else {
                    format!("{{\"oneOf\":{}}}", json_array(references))
                };
                responses.push(format!(
                    "{}:{{\"description\":{},\"content\":{{\"application/json\":{{\"schema\":{}}}}}}}",
                    json_string(&status.to_string()),
                    json_string(&names.join(" or ")),
                    failure_schema
                ));
            }
            responses.sort();
            let request_body = route.input.as_ref().map_or_else(
                || "null".to_owned(),
                |input| {
                    format!(
                        "{{\"required\":true,\"content\":{{\"application/json\":{{\"schema\":{}}}}}}}",
                        self.openapi_type_schema(input)
                    )
                },
            );
            let parameters = json_array(route.path_fields.iter().map(|(name, field_type)| {
                format!(
                    "{{\"name\":{},\"in\":\"path\",\"required\":true,\"schema\":{}}}",
                    json_string(name),
                    self.openapi_type_schema(field_type)
                )
            }));
            let security = if route.public {
                "[]".to_owned()
            } else {
                json_array(
                    strategies
                        .iter()
                        .map(|s| format!("{{{}:[]}}", json_string(&s.name.text))),
                )
            };
            let operation = format!(
                "{}:{{\"operationId\":{},\"parameters\":{},\"requestBody\":{},\"responses\":{{{}}},\"security\":{},\"x-jadpo-fresh-authority\":{}}}",
                json_string(&route.method.to_ascii_lowercase()),
                json_string(route.callable.as_deref().unwrap_or(&route.key)),
                parameters,
                request_body,
                responses.join(","),
                security,
                route.fresh_authority
            );
            paths.entry(route.path.clone()).or_default().push(operation);
        }

        let mut schemas = Vec::new();
        for (name, declaration) in &self.types {
            schemas.push(format!(
                "{}:{}",
                json_string(name),
                self.openapi_scalar_schema(declaration)
            ));
        }
        for (name, declaration) in &self.enums {
            schemas.push(format!(
                "{}:{}",
                json_string(name),
                self.openapi_enum_schema(declaration)
            ));
        }
        for (name, declaration) in self
            .records
            .iter()
            .filter(|(name, _)| !name.starts_with("__jadpo_"))
        {
            schemas.push(format!(
                "{}:{}",
                json_string(name),
                self.openapi_record_schema(declaration)
            ));
        }
        for name in self.public_field_schema_names() {
            let field = self.fields[&name];
            schemas.push(format!(
                "{}:{}",
                json_string(&name),
                self.openapi_field_schema(field)
            ));
        }
        for contract in &self.project.failures.contracts {
            schemas.push(format!(
                "{}:{}",
                json_string(&contract.name),
                self.openapi_failure_schema(contract)
            ));
        }
        schemas.sort();

        let components = format!(
            "{{\"schemas\":{{{}}},\"securitySchemes\":{{{}}}}}",
            schemas.join(","),
            security_schemes
        );
        format!(
            "{{\"openapi\":\"3.1.0\",\"info\":{{\"title\":\"Application API\",\"version\":\"0.0.0\"}},\"paths\":{{{}}},\"components\":{}}}",
            paths.iter().map(|(path, operations)| format!("{}:{{{}}}", json_string(path), operations.join(","))).collect::<Vec<_>>().join(","), components
        )
    }

    fn route_failures(&self, route_key: &str) -> Vec<RouteFailureView<'_>> {
        self.project
            .failures
            .routes
            .iter()
            .filter(|route| route.route == route_key)
            .filter_map(|route| {
                self.project
                    .failures
                    .contracts
                    .iter()
                    .find(|contract| contract.name == route.failure)
                    .map(|contract| RouteFailureView {
                        name: &route.failure,
                        code: &contract.code,
                        http_status: route.http_status,
                    })
            })
            .collect()
    }

    fn openapi_field_schema(&self, field: &FieldDeclaration) -> String {
        let parent = self.openapi_type_schema(&type_name(&field.field_type));
        let constraints = openapi_constraint_parts(&field.constraints);
        if constraints.is_empty() {
            parent
        } else {
            format!("{{\"allOf\":[{parent},{{{}}}]}}", constraints.join(","))
        }
    }

    fn public_field_schema_names(&self) -> BTreeSet<String> {
        let is_internal = |name: &str| {
            self.failure_declarations
                .keys()
                .any(|failure| name.starts_with(&format!("{failure}.internal.")))
        };
        let mut names = self
            .fields
            .keys()
            .filter(|name| !is_internal(name) && !name.starts_with("__jadpo_"))
            .cloned()
            .collect::<BTreeSet<_>>();
        for declaration in self.types.values() {
            self.collect_field_schema_names(&type_name(&declaration.parent), &mut names);
        }
        for route in &self.routes {
            for reference in route
                .input
                .iter()
                .chain(route.output.iter())
                .chain(route.path_fields.iter().map(|(_, reference)| reference))
            {
                self.collect_field_schema_names(reference, &mut names);
            }
        }
        // Include internal value contracts only when a public declaration
        // explicitly references them. Follow chains to keep every $ref valid.
        loop {
            let before = names.len();
            for name in names.clone() {
                if let Some(field) = self.fields.get(&name) {
                    self.collect_field_schema_names(&type_name(&field.field_type), &mut names);
                }
            }
            if names.len() == before {
                return names;
            }
        }
    }

    fn collect_field_schema_names(&self, name: &str, output: &mut BTreeSet<String>) {
        let base = name.trim_end_matches('?');
        if self.fields.contains_key(base) && !base.starts_with("__jadpo_") {
            output.insert(base.to_owned());
        }
        if let Some((_, arguments)) = base.split_once('<') {
            if let Some(arguments) = arguments.strip_suffix('>') {
                for argument in split_generic_arguments(arguments) {
                    self.collect_field_schema_names(argument, output);
                }
            }
        }
    }

    fn openapi_scalar_schema(&self, declaration: &TypeDeclaration) -> String {
        let parent = self.openapi_type_schema(&type_name(&declaration.parent));
        let constraints = openapi_constraint_parts(&declaration.constraints);
        if constraints.is_empty() {
            parent
        } else {
            format!("{{\"allOf\":[{parent},{{{}}}]}}", constraints.join(","))
        }
    }

    fn openapi_enum_schema(&self, declaration: &EnumDeclaration) -> String {
        if declaration
            .variants
            .iter()
            .any(|variant| !variant.fields.is_empty())
        {
            let variants = declaration.variants.iter().map(|variant| {
                let properties = std::iter::once(format!(
                    "\"tag\":{{\"type\":\"string\",\"const\":{}}}",
                    json_string(&variant.name.text)
                ))
                .chain(variant.fields.iter().map(|field| {
                    format!(
                        "{}:{}",
                        json_string(&field.name.text),
                        self.openapi_field_schema(field)
                    )
                }))
                .collect::<Vec<_>>()
                .join(",");
                let required = std::iter::once("tag")
                    .chain(
                        variant
                            .fields
                            .iter()
                            .filter(|field| !field.optional)
                            .map(|field| field.name.text.as_str()),
                    );
                format!(
                    "{{\"type\":\"object\",\"additionalProperties\":false,\"properties\":{{{properties}}},\"required\":{}}}",
                    json_string_array(required)
                )
            });
            return format!(
                "{{\"oneOf\":{},\"discriminator\":{{\"propertyName\":\"tag\"}}}}",
                json_array(variants)
            );
        }
        format!(
            "{{\"type\":\"string\",\"enum\":{}}}",
            json_string_array(
                declaration
                    .variants
                    .iter()
                    .map(|variant| variant.name.text.as_str())
            )
        )
    }

    fn openapi_record_schema(&self, declaration: &RecordDeclaration) -> String {
        let properties = declaration.fields.iter().map(|field| {
            format!(
                "{}:{}",
                json_string(&field.name.text),
                self.openapi_field_schema(field)
            )
        });
        let required = declaration
            .fields
            .iter()
            .filter(|field| !field.optional)
            .map(|field| field.name.text.as_str());
        format!(
            "{{\"type\":\"object\",\"additionalProperties\":false,\"properties\":{{{}}},\"required\":{}}}",
            properties.collect::<Vec<_>>().join(","),
            json_string_array(required)
        )
    }

    fn openapi_failure_schema(&self, contract: &jadpo_semantic::FailureContract) -> String {
        let mut error_properties = vec![
            format!(
                "\"code\":{{\"type\":\"string\",\"const\":{}}}",
                json_string(&contract.code)
            ),
            "\"request_id\":{\"type\":\"string\"}".to_owned(),
        ];
        let mut required = vec!["code", "request_id"];
        if let Some(message) = &contract.message {
            error_properties.push(format!(
                "\"message\":{{\"type\":\"string\",\"const\":{}}}",
                json_string(message)
            ));
            required.push("message");
        } else {
            error_properties.push("\"message\":{\"type\":\"string\"}".to_owned());
            required.push("message");
        }
        if !contract.public_fields.is_empty() {
            let declaration = self
                .failure_declarations
                .get(&contract.name)
                .expect("checked failure contract has its authored declaration");
            let properties = declaration
                .public_fields
                .iter()
                .map(|field| {
                    format!(
                        "{}:{}",
                        json_string(&field.name.text),
                        self.openapi_field_schema(field)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            let names = json_string_array(
                declaration
                    .public_fields
                    .iter()
                    .map(|field| field.name.text.as_str()),
            );
            error_properties.push(format!("\"details\":{{\"type\":\"object\",\"additionalProperties\":false,\"properties\":{{{properties}}},\"required\":{names}}}"));
            required.push("details");
        }
        error_properties.sort();
        required.sort();
        format!(
            "{{\"type\":\"object\",\"additionalProperties\":false,\"properties\":{{\"error\":{{\"type\":\"object\",\"additionalProperties\":false,\"properties\":{{{}}},\"required\":{}}}}},\"required\":[\"error\"]}}",
            error_properties.join(","),
            json_string_array(required.into_iter())
        )
    }

    fn openapi_type_schema(&self, name: &str) -> String {
        let base = name.trim_end_matches('?');
        let nullable =
            base.len() != name.len() || self.project.semantics.nullable_types.contains(base);
        let schema = if let Some(field) = self
            .fields
            .get(base)
            .filter(|_| base.starts_with("__jadpo_"))
        {
            self.openapi_field_schema(field)
        } else if self.fields.contains_key(base) {
            format!(
                "{{\"$ref\":{}}}",
                json_string(&format!("#/components/schemas/{base}"))
            )
        } else if let Some(record) = base
            .strip_suffix(".Ref")
            .and_then(|owner| self.records.get(owner))
        {
            let identity = record
                .dossier
                .as_ref()
                .map(|dossier| dossier.identity.text.as_str())
                .or_else(|| {
                    record
                        .fields
                        .iter()
                        .find(|field| field.persistence.contains(&PersistenceModifier::Identity))
                        .map(|field| field.name.text.as_str())
                });
            identity
                .and_then(|identity| {
                    record
                        .fields
                        .iter()
                        .find(|field| field.name.text == identity)
                })
                .map_or_else(|| "{}".to_owned(), |field| self.openapi_field_schema(field))
        } else if let Some(item) = base
            .strip_prefix("List<")
            .and_then(|value| value.strip_suffix('>'))
        {
            format!(
                "{{\"type\":\"array\",\"items\":{}}}",
                self.openapi_type_schema(item)
            )
        } else if let Some(item) = base
            .strip_prefix("Set<")
            .and_then(|value| value.strip_suffix('>'))
        {
            format!(
                "{{\"type\":\"array\",\"uniqueItems\":true,\"items\":{}}}",
                self.openapi_type_schema(item)
            )
        } else if let Some(arguments) = base
            .strip_prefix("Map<")
            .and_then(|value| value.strip_suffix('>'))
        {
            let value = split_generic_arguments(arguments)
                .get(1)
                .map_or_else(|| "{}".to_owned(), |value| self.openapi_type_schema(value));
            format!("{{\"type\":\"object\",\"additionalProperties\":{value}}}")
        } else {
            let resolved = self.schema_declaration(base);
            if let Some(record) = self
                .records
                .get(&resolved)
                .filter(|_| resolved.starts_with("__jadpo_"))
            {
                self.openapi_record_schema(record)
            } else if self.types.contains_key(&resolved)
                || self.enums.contains_key(&resolved)
                || self.records.contains_key(&resolved)
            {
                format!(
                    "{{\"$ref\":{}}}",
                    json_string(&format!("#/components/schemas/{resolved}"))
                )
            } else {
                format!("{{{}}}", scalar_schema_parts(&resolved).join(","))
            }
        };
        if nullable {
            format!("{{\"anyOf\":[{schema},{{\"type\":\"null\"}}]}}")
        } else {
            schema
        }
    }

    fn reference_is_nullable(&self, reference: &TypeReference) -> bool {
        reference.nullable
            || self.project.semantics.nullable_types.contains(
                &reference
                    .path
                    .iter()
                    .map(|part| part.text.as_str())
                    .collect::<Vec<_>>()
                    .join("."),
            )
    }

    fn schema_declaration(&self, name: &str) -> String {
        if matches!(name, "Email" | "Url" | "IpAddress") {
            return name.to_owned();
        }
        let mut current = name.to_owned();
        let mut visited = BTreeSet::new();
        while visited.insert(current.clone()) {
            if matches!(current.as_str(), "Email" | "Url" | "IpAddress") {
                return current;
            }
            if self.types.contains_key(&current)
                || self.enums.contains_key(&current)
                || self.records.contains_key(&current)
            {
                return current;
            }
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
}

struct RouteFailureView<'a> {
    name: &'a str,
    code: &'a str,
    http_status: u16,
}

fn field_plan_json(field: &FieldDeclaration, nullable: bool) -> String {
    format!(
        "{{\"name\":{},\"type\":{},\"optional\":{},\"nullable\":{},\"constraints\":{}}}",
        json_string(&field.name.text),
        json_string(&type_name(&field.field_type)),
        field.optional,
        nullable,
        constraints_json(&field.constraints)
    )
}

fn constraints_json(constraints: &[Constraint]) -> String {
    json_array(constraints.iter().map(|constraint| {
        format!(
            "{{\"kind\":{},\"value\":{}}}",
            json_string(constraint_name(constraint.kind)),
            literal_json(constraint)
        )
    }))
}

fn literal_json(constraint: &Constraint) -> String {
    match constraint.value.kind {
        LiteralKind::Integer | LiteralKind::Decimal | LiteralKind::Boolean => {
            constraint.value.text.clone()
        }
        LiteralKind::String => json_string(unquote(&constraint.value.text)),
        LiteralKind::None => "null".to_owned(),
    }
}

fn openapi_constraint_parts(constraints: &[Constraint]) -> Vec<String> {
    constraints
        .iter()
        .map(|constraint| {
            let key = match constraint.kind {
                ConstraintKind::Min => "minimum",
                ConstraintKind::Max => "maximum",
                ConstraintKind::MinLength => "minLength",
                ConstraintKind::MaxLength => "maxLength",
                ConstraintKind::Pattern => "pattern",
                ConstraintKind::Format => "format",
            };
            format!("{}:{}", json_string(key), literal_json(constraint))
        })
        .collect()
}

fn scalar_schema_parts(name: &str) -> Vec<String> {
    match name {
        "Bool" => vec!["\"type\":\"boolean\"".to_owned()],
        "Int" => vec!["\"type\":\"integer\"".to_owned()],
        "Decimal" => vec!["\"type\":\"number\"".to_owned()],
        "Uuid" => vec![
            "\"type\":\"string\"".to_owned(),
            "\"format\":\"uuid\"".to_owned(),
        ],
        "Instant" => vec![
            "\"type\":\"string\"".to_owned(),
            "\"format\":\"date-time\"".to_owned(),
        ],
        "CalendarDate" => vec![
            "\"type\":\"string\"".to_owned(),
            "\"format\":\"date\"".to_owned(),
        ],
        "Time" => vec![
            "\"type\":\"object\"".to_owned(),
            "\"additionalProperties\":false".to_owned(),
            "\"required\":[\"instant\",\"zone\"]".to_owned(),
            "\"properties\":{\"instant\":{\"type\":\"string\",\"format\":\"date-time\"},\"zone\":{\"type\":\"string\"}}".to_owned(),
        ],
        "Zone" | "Locale" | "PresentationText" => vec!["\"type\":\"string\"".to_owned()],
        "Duration" => vec![
            "\"type\":\"string\"".to_owned(),
            "\"format\":\"duration\"".to_owned(),
        ],
        "Bytes" => vec![
            "\"type\":\"string\"".to_owned(),
            "\"contentEncoding\":\"base64\"".to_owned(),
        ],
        "Email" => vec![
            "\"type\":\"string\"".to_owned(),
            "\"format\":\"email\"".to_owned(),
            "\"maxLength\":254".to_owned(),
        ],
        "Url" => vec![
            "\"type\":\"string\"".to_owned(),
            "\"format\":\"uri\"".to_owned(),
        ],
        "IpAddress" => vec![
            "\"type\":\"string\"".to_owned(),
            "\"format\":\"ip\"".to_owned(),
        ],
        _ => vec!["\"type\":\"string\"".to_owned()],
    }
}

fn split_generic_arguments(arguments: &str) -> Vec<&str> {
    let mut depth = 0usize;
    let mut start = 0usize;
    let mut values = Vec::new();
    for (index, character) in arguments.char_indices() {
        match character {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                values.push(arguments[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    values.push(arguments[start..].trim());
    values
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

fn method_name(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Delete => "DELETE",
    }
}

fn record_kind(kind: RecordKind) -> &'static str {
    match kind {
        RecordKind::Entity | RecordKind::Value | RecordKind::Input | RecordKind::Output => "object",
    }
}

fn constraint_name(kind: ConstraintKind) -> &'static str {
    match kind {
        ConstraintKind::Min => "min",
        ConstraintKind::Max => "max",
        ConstraintKind::MinLength => "min_length",
        ConstraintKind::MaxLength => "max_length",
        ConstraintKind::Pattern => "pattern",
        ConstraintKind::Format => "format",
    }
}

fn json_optional_string(value: Option<&str>) -> String {
    value.map(json_string).unwrap_or_else(|| "null".to_owned())
}

fn json_string_array<'a>(values: impl IntoIterator<Item = &'a str>) -> String {
    json_array(values.into_iter().map(json_string))
}

fn json_array(values: impl IntoIterator<Item = String>) -> String {
    format!("[{}]", values.into_iter().collect::<Vec<_>>().join(","))
}

fn json_string(value: &str) -> String {
    format!("\"{}\"", escape_json(value))
}

fn escape_json(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                escaped.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => escaped.push(character),
        }
    }
    escaped
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::{derive_artifacts, write_artifacts, GeneratedArtifact};
    use crate::analyze_project;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn repository_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("compiler crate should be inside the repository")
    }

    #[test]
    fn derives_the_complete_seed_artifact_set() {
        let seed = repository_root().join("examples/jadpo-seed");
        let analyzed = analyze_project(&seed).expect("seed should be analyzable");
        let artifacts = derive_artifacts(&seed, &analyzed);
        let paths = artifacts
            .iter()
            .map(|artifact| artifact.relative_path)
            .collect::<Vec<_>>();

        assert_eq!(
            paths,
            vec![
                "app.meta.json",
                "inventory/routes.json",
                "inventory/callables.json",
                "audit/failures.json",
                "audit/entities.json",
                "audit/transactions.json",
                "audit/configuration.json",
                "audit/policy.json",
                "validators/plan.json",
                "compatibility/public-failure-codes.json",
                "openapi/openapi.json",
                "diagnostics/catalogue.json",
                "diagnostics/reference.md",
            ]
        );
        assert!(artifact(&artifacts, "inventory/routes.json").contains("\"auth\":\"none\""));
        assert!(artifact(&artifacts, "diagnostics/catalogue.json")
            .contains("\"ruleId\":\"failure.attempt_required\""));
        assert!(
            artifact(&artifacts, "audit/failures.json").contains("\"internal_to_client\":false")
        );
        assert!(artifact(&artifacts, "openapi/openapi.json").contains("\"422\""));
    }

    #[test]
    fn exposes_enum_variants_to_validators_and_openapi() {
        let fixture =
            repository_root().join("tests/compile/pass/53_plain_enum_exhaustive_match.jadpo");
        let analyzed = analyze_project(&fixture).expect("enum fixture should be analyzable");
        let artifacts = derive_artifacts(&fixture, &analyzed);

        assert!(artifact(&artifacts, "validators/plan.json").contains(
            "\"enums\":[{\"name\":\"DeliveryState\",\"tagged\":false,\"variants\":[\"pending\",\"sent\",\"failed\"]"
        ));
        assert!(artifact(&artifacts, "openapi/openapi.json").contains(
            "\"DeliveryState\":{\"type\":\"string\",\"enum\":[\"pending\",\"sent\",\"failed\"]}"
        ));
    }

    #[test]
    fn exposes_proved_restricted_field_reads_in_policy_audit() {
        let fixture = repository_root().join("tests/compile/pass/134_policy_safe_projection.jadpo");
        let analyzed = analyze_project(&fixture).expect("policy fixture should be analyzable");
        let artifacts = derive_artifacts(&fixture, &analyzed);
        let policy = artifact(&artifacts, "audit/policy.json");

        assert!(policy.contains("\"operation\":\"PrivateNote.read_private_label\""));
        assert!(policy.contains("\"restricted_field_reads\":[{\"entity\":\"PrivateNote\",\"field\":\"private_label\",\"subjects\":[\"NoteRole.owner\"]"));
        assert!(policy.contains("\"judgement\":\"proved\""));
    }

    #[test]
    fn exposes_nested_objects_collections_and_validated_prelude_types() {
        let fixture =
            repository_root().join("tests/compile/pass/108_unified_types_and_persistence.jadpo");
        let analyzed = analyze_project(&fixture).expect("unified type fixture should analyze");
        let artifacts = derive_artifacts(&fixture, &analyzed);
        let openapi = artifact(&artifacts, "openapi/openapi.json");
        let validators = artifact(&artifacts, "validators/plan.json");

        assert!(openapi.contains("\"addresses\":{\"type\":\"array\""));
        assert!(openapi.contains("\"format\":\"email\""));
        assert!(openapi.contains("\"format\":\"uri\""));
        assert!(openapi.contains("\"format\":\"ip\""));
        assert!(!openapi.contains("__jadpo_"));
        assert!(validators.contains("\"kind\":\"object\",\"persistent\":true"));
        assert!(!validators.contains("\"kind\":\"output\""));
    }

    #[test]
    fn exposes_typed_route_paths_and_behavior_in_inventory_and_openapi() {
        let fixture = repository_root().join("tests/compile/pass/59_p106_failure_route.jadpo");
        let analyzed = analyze_project(&fixture).expect("route fixture should be analyzable");
        let artifacts = derive_artifacts(&fixture, &analyzed);
        let inventory = artifact(&artifacts, "inventory/routes.json");
        assert!(inventory
            .contains("\"path_fields\":[{\"name\":\"customer_id\",\"type\":\"Customer.id\"}]"));
        assert!(inventory.contains("\"behavior\":\"run\""));

        let openapi = artifact(&artifacts, "openapi/openapi.json");
        assert!(openapi.contains("\"name\":\"customer_id\",\"in\":\"path\",\"required\":true"));
        assert!(openapi.contains("\"format\":\"uuid\""));

        let inline_fixture =
            repository_root().join("tests/compile/pass/66_inline_action_failure_surface.jadpo");
        let inline = analyze_project(&inline_fixture).expect("inline route should be analyzable");
        let inline_artifacts = derive_artifacts(&inline_fixture, &inline);
        assert!(artifact(&inline_artifacts, "inventory/routes.json")
            .contains("\"name\":\"Refused\",\"code\":\"refused\",\"http_status\":422"));
        assert!(artifact(&inline_artifacts, "openapi/openapi.json")
            .contains("\"422\":{\"description\":\"Refused\""));
    }

    #[test]
    fn exposes_tagged_sum_payloads_and_discriminator_to_openapi() {
        let fixture = repository_root().join("tests/compile/pass/56_tagged_sum_match.jadpo");
        let analyzed = analyze_project(&fixture).expect("tagged-sum fixture should analyze");
        let artifacts = derive_artifacts(&fixture, &analyzed);

        assert!(artifact(&artifacts, "validators/plan.json")
            .contains("\"name\":\"PaymentOutcome\",\"tagged\":true"));
        let openapi = artifact(&artifacts, "openapi/openapi.json");
        assert!(openapi.contains("\"discriminator\":{\"propertyName\":\"tag\"}"));
        assert!(openapi.contains("\"const\":\"paid\""));
        assert!(openapi.contains("\"receipt_id\""));
    }

    #[test]
    fn repeated_emission_is_byte_identical() {
        let temp = temporary_project("repeat");
        fs::create_dir_all(&temp).expect("temp project should be created");
        fs::copy(
            repository_root().join("examples/jadpo-seed/app.jadpo"),
            temp.join("app.jadpo"),
        )
        .expect("seed source should be copied");

        let analyzed = analyze_project(&temp).expect("temp seed should be analyzable");
        let artifacts = derive_artifacts(&temp, &analyzed);
        let first_root = write_artifacts(&temp, &artifacts).expect("first emission should work");
        let first = read_outputs(&first_root, &artifacts);
        let second_root = write_artifacts(&temp, &artifacts).expect("second emission should work");
        let second = read_outputs(&second_root, &artifacts);

        assert_eq!(first, second);
        fs::remove_dir_all(&temp).expect("temp project should be removable");
    }

    #[test]
    fn failed_staging_preserves_the_last_complete_build() {
        let temp = temporary_project("failure");
        fs::create_dir_all(&temp).expect("temp project should be created");
        let baseline = [GeneratedArtifact {
            relative_path: "target/app.ts",
            contents: "baseline\n".to_owned(),
        }];
        let output = write_artifacts(&temp, &baseline).expect("baseline should be written");

        let invalid = [
            GeneratedArtifact {
                relative_path: "conflict",
                contents: "file blocks directory\n".to_owned(),
            },
            GeneratedArtifact {
                relative_path: "conflict/child",
                contents: "cannot be written\n".to_owned(),
            },
        ];
        let diagnostic = write_artifacts(&temp, &invalid).expect_err("staging should fail");

        assert_eq!(diagnostic.code, "JADPO_ARTIFACT_WRITE_FAILED");
        assert_eq!(
            fs::read_to_string(output.join("target/app.ts"))
                .expect("last complete build should remain"),
            "baseline\n"
        );
        assert!(!output.join("conflict").exists());
        assert!(fs::read_dir(&temp)
            .expect("temp project should be readable")
            .all(|entry| !entry
                .expect("entry should be readable")
                .file_name()
                .to_string_lossy()
                .starts_with(".jadpo-build-stage.")));
        fs::remove_dir_all(&temp).expect("temp project should be removable");
    }

    #[test]
    fn metadata_is_independent_of_relative_or_absolute_invocation() {
        let seed = repository_root().join("examples/jadpo-seed");
        let absolute = analyze_project(&seed).expect("absolute seed should be analyzable");
        let absolute_artifacts = derive_artifacts(&seed, &absolute);

        let current = std::env::current_dir().expect("current directory should be available");
        let relative_path = relative_path(&current, &seed);
        let relative = analyze_project(&relative_path).expect("relative seed should be analyzable");
        let relative_artifacts = derive_artifacts(&relative_path, &relative);

        assert_eq!(
            artifact(&absolute_artifacts, "app.meta.json"),
            artifact(&relative_artifacts, "app.meta.json")
        );
    }

    fn artifact<'a>(artifacts: &'a [super::GeneratedArtifact], path: &str) -> &'a str {
        &artifacts
            .iter()
            .find(|artifact| artifact.relative_path == path)
            .expect("artifact should exist")
            .contents
    }

    fn read_outputs(root: &Path, artifacts: &[super::GeneratedArtifact]) -> Vec<(String, Vec<u8>)> {
        artifacts
            .iter()
            .map(|artifact| {
                (
                    artifact.relative_path.to_owned(),
                    fs::read(root.join(artifact.relative_path))
                        .expect("artifact should be readable"),
                )
            })
            .collect()
    }

    fn temporary_project(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "jadpo-compiler-artifacts-{}-{name}",
            std::process::id(),
        ))
    }

    fn relative_path(from: &Path, to: &Path) -> PathBuf {
        let from = from
            .canonicalize()
            .expect("working directory should canonicalize");
        let to = to.canonicalize().expect("seed path should canonicalize");
        let from = from.components().collect::<Vec<_>>();
        let to = to.components().collect::<Vec<_>>();
        let shared = from
            .iter()
            .zip(&to)
            .take_while(|(left, right)| left == right)
            .count();
        let mut relative = PathBuf::new();
        for _ in shared..from.len() {
            relative.push("..");
        }
        for component in &to[shared..] {
            relative.push(component.as_os_str());
        }
        relative
    }
}
