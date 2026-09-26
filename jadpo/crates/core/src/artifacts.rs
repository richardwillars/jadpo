use crate::AnalyzedProject;
use jadpo_diagnostics::Diagnostic;
use jadpo_semantic::checked_manifest_json;
use jadpo_syntax::{
    CallableKind, Constraint, ConstraintKind, Declaration, EnumDeclaration, FieldDeclaration,
    HttpMethod, LiteralKind, RecordDeclaration, RecordKind, TypeDeclaration, TypeReference,
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
    vec![
        artifact("app.meta.json", metadata),
        artifact("inventory/routes.json", model.routes_json()),
        artifact("inventory/callables.json", model.callables_json()),
        artifact("audit/failures.json", model.failure_audit_json()),
        artifact("validators/plan.json", model.validator_plan_json()),
        artifact(
            "compatibility/public-failure-codes.json",
            model.public_failure_compatibility_json(),
        ),
        artifact("openapi/openapi.json", model.openapi_json()),
    ]
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
    checked_manifest_json(&graph, &typing, &project.failures)
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

    fs::create_dir(&staging_root).map_err(|error| {
        Diagnostic::error(
            "JADPO_ARTIFACT_STAGE_FAILED",
            format!(
                "could not create artifact staging directory {}: {error}",
                staging_root.display()
            ),
        )
    })?;

    for artifact in artifacts {
        let destination = staging_root.join(artifact.relative_path);
        if let Some(parent) = destination.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                let _ = fs::remove_dir_all(&staging_root);
                return Err(Diagnostic::error(
                    "JADPO_ARTIFACT_WRITE_FAILED",
                    format!("could not create {}: {error}", parent.display()),
                ));
            }
        }
        if let Err(error) = fs::write(&destination, &artifact.contents) {
            let _ = fs::remove_dir_all(&staging_root);
            return Err(Diagnostic::error(
                "JADPO_ARTIFACT_WRITE_FAILED",
                format!("could not write {}: {error}", destination.display()),
            ));
        }
    }

    let had_previous = output_root.exists();
    if had_previous {
        if let Err(error) = fs::rename(&output_root, &backup_root) {
            let _ = fs::remove_dir_all(&staging_root);
            return Err(Diagnostic::error(
                "JADPO_ARTIFACT_PROMOTE_FAILED",
                format!(
                    "could not preserve previous build {}: {error}",
                    output_root.display()
                ),
            ));
        }
    }

    if let Err(error) = fs::rename(&staging_root, &output_root) {
        if had_previous {
            let _ = fs::rename(&backup_root, &output_root);
        }
        let _ = fs::remove_dir_all(&staging_root);
        return Err(Diagnostic::error(
            "JADPO_ARTIFACT_PROMOTE_FAILED",
            format!(
                "could not promote staged build to {}: {error}",
                output_root.display()
            ),
        ));
    }

    if had_previous {
        fs::remove_dir_all(&backup_root).map_err(|error| {
            Diagnostic::error(
                "JADPO_ARTIFACT_CLEANUP_FAILED",
                format!(
                    "promoted build but could not remove prior revision {}: {error}",
                    backup_root.display()
                ),
            )
        })?;
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
    routes: Vec<RouteModel>,
    callables: Vec<CallableModel>,
}

#[derive(Clone)]
struct RouteModel {
    key: String,
    method: &'static str,
    path: String,
    public: bool,
    input: Option<String>,
    output: Option<String>,
    callable: Option<String>,
}

#[derive(Clone)]
struct CallableModel {
    name: String,
    kind: &'static str,
    parameters: Vec<(String, String)>,
    output: String,
    failures: Vec<String>,
}

impl<'project> ArtifactModel<'project> {
    fn new(project: &'project AnalyzedProject) -> Self {
        let mut types = BTreeMap::new();
        let mut enums = BTreeMap::new();
        let mut records = BTreeMap::new();
        let mut routes = Vec::new();
        let mut callables = Vec::new();

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
                        });
                    }
                    Declaration::Route(declaration) => {
                        let method = method_name(declaration.method);
                        routes.push(RouteModel {
                            key: format!("{method} {}", declaration.path),
                            method,
                            path: declaration.path.clone(),
                            public: declaration.public,
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
                        });
                    }
                    Declaration::Failure(_) | Declaration::Test(_) => {}
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
            routes,
            callables,
        }
    }

    fn routes_json(&self) -> String {
        let routes = self
            .routes
            .iter()
            .map(|route| {
                let failures = self.route_failures(&route.key);
                format!(
                    "{{\"route\":{},\"method\":{},\"path\":{},\"auth\":{},\"input\":{},\"output\":{},\"callable\":{},\"failures\":{}}}",
                    json_string(&route.key),
                    json_string(route.method),
                    json_string(&route.path),
                    json_string(if route.public { "public_explicit" } else { "authenticated_default" }),
                    json_optional_string(route.input.as_deref()),
                    json_optional_string(route.output.as_deref()),
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
                "{{\"name\":{},\"kind\":{},\"parameters\":{},\"output\":{},\"failures\":{}}}",
                json_string(&callable.name),
                json_string(callable.kind),
                json_array(parameters),
                json_string(&callable.output),
                json_string_array(callable.failures.iter().map(String::as_str))
            )
        });
        format!(
            "{{\"schema_version\":1,\"callables\":{}}}",
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
            let fields = declaration.fields.iter().map(field_plan_json);
            format!(
                "{{\"name\":{},\"kind\":{},\"closed_shape\":true,\"fields\":{}}}",
                json_string(name),
                json_string(record_kind(declaration.kind)),
                json_array(fields)
            )
        });
        let enums = self.enums.iter().map(|(name, declaration)| {
            let payloads = declaration.variants.iter().map(|variant| {
                format!(
                    "{{\"name\":{},\"fields\":{}}}",
                    json_string(&variant.name.text),
                    json_array(variant.fields.iter().map(field_plan_json))
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

    fn openapi_json(&self) -> String {
        let paths = self.routes.iter().map(|route| {
            let mut responses = Vec::new();
            if let Some(output) = &route.output {
                responses.push(format!(
                    "\"200\":{{\"description\":\"Success\",\"content\":{{\"application/json\":{{\"schema\":{}}}}}}}",
                    self.openapi_type_schema(output)
                ));
            }
            for failure in self.route_failures(&route.key) {
                let failure_schema = format!(
                    "{{\"$ref\":{}}}",
                    json_string(&format!("#/components/schemas/{}", failure.name))
                );
                responses.push(format!(
                    "{}:{{\"description\":{},\"content\":{{\"application/json\":{{\"schema\":{}}}}}}}",
                    json_string(&failure.http_status.to_string()),
                    json_string(failure.name),
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
            format!(
                "{}:{{{}:{{\"operationId\":{},\"requestBody\":{},\"responses\":{{{}}}}}}}",
                json_string(&route.path),
                json_string(&route.method.to_ascii_lowercase()),
                json_string(route.callable.as_deref().unwrap_or(&route.key)),
                request_body,
                responses.join(",")
            )
        });

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
        for (name, declaration) in &self.records {
            schemas.push(format!(
                "{}:{}",
                json_string(name),
                self.openapi_record_schema(declaration)
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

        let components = format!("{{\"schemas\":{{{}}}}}", schemas.join(","));
        format!(
            "{{\"openapi\":\"3.1.0\",\"info\":{{\"title\":\"Application API\",\"version\":\"0.0.0\"}},\"paths\":{{{}}},\"components\":{}}}",
            paths.collect::<Vec<_>>().join(","), components
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

    fn openapi_scalar_schema(&self, declaration: &TypeDeclaration) -> String {
        let mut properties = scalar_schema_parts(&type_name(&declaration.parent));
        properties.extend(openapi_constraint_parts(&declaration.constraints));
        format!("{{{}}}", properties.join(","))
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
                        self.openapi_type_schema(&type_name(&field.field_type))
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
                self.openapi_type_schema(&type_name(&field.field_type))
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
        }
        for field in &contract.public_fields {
            error_properties.push(format!("{}:{{}}", json_string(field)));
            required.push(field);
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
        let nullable = base.len() != name.len();
        let resolved = self.schema_declaration(base);
        let schema = if self.types.contains_key(&resolved)
            || self.enums.contains_key(&resolved)
            || self.records.contains_key(&resolved)
        {
            format!(
                "{{\"$ref\":{}}}",
                json_string(&format!("#/components/schemas/{resolved}"))
            )
        } else {
            format!("{{{}}}", scalar_schema_parts(&resolved).join(","))
        };
        if nullable {
            format!("{{\"anyOf\":[{schema},{{\"type\":\"null\"}}]}}")
        } else {
            schema
        }
    }

    fn schema_declaration(&self, name: &str) -> String {
        let mut current = name.to_owned();
        let mut visited = BTreeSet::new();
        while visited.insert(current.clone()) {
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

fn field_plan_json(field: &FieldDeclaration) -> String {
    format!(
        "{{\"name\":{},\"type\":{},\"optional\":{},\"nullable\":{},\"constraints\":{}}}",
        json_string(&field.name.text),
        json_string(&type_name(&field.field_type)),
        field.optional,
        field.field_type.nullable,
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
        "DateTime" => vec![
            "\"type\":\"string\"".to_owned(),
            "\"format\":\"date-time\"".to_owned(),
        ],
        _ => vec!["\"type\":\"string\"".to_owned()],
    }
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
        RecordKind::Entity => "entity",
        RecordKind::Value => "value",
        RecordKind::Input => "input",
        RecordKind::Output => "output",
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
                "validators/plan.json",
                "compatibility/public-failure-codes.json",
                "openapi/openapi.json",
            ]
        );
        assert!(
            artifact(&artifacts, "inventory/routes.json").contains("\"auth\":\"public_explicit\"")
        );
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
