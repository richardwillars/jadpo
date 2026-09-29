use jadpo_diagnostics::{Diagnostic, DiagnosticFact, SourceSpan};
use jadpo_syntax::{
    Block, CallableKind, Declaration, Expression, HttpMethod, ParsedSyntax, PersistenceModifier,
    RecordKind, ReferenceDeleteAction, Statement, TextRange, TypeReference,
};
use std::collections::{BTreeMap, BTreeSet};

mod failurecheck;
mod typecheck;

pub use failurecheck::{
    check_failures, CallableFailureSet, FailureCheckResult, FailureContract, RouteFailure,
};
pub use typecheck::{check_types, ClockRead, InferredExpression, TypeCheckResult};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NodeId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NodeKind {
    PreludeType,
    StandardNamespace,
    StandardFunction,
    StandardValue,
    StandardFailure,
    Application,
    Locales,
    AuthenticationStrategy,
    CredentialSlot,
    CredentialValidation,
    CredentialClaimMapping,
    PrincipalResolution,
    PrincipalMapping,
    Principal,
    PrincipalVariant,
    Configuration,
    ConfigurationField,
    Type,
    Enum,
    EnumVariant,
    Entity,
    EntityReference,
    Value,
    Input,
    Output,
    Field,
    Relationship,
    PersistenceConstraint,
    Failure,
    Function,
    Action,
    Query,
    Fixture,
    Test,
    Route,
}

impl NodeKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PreludeType => "prelude_type",
            Self::StandardNamespace => "standard_namespace",
            Self::StandardFunction => "standard_function",
            Self::StandardValue => "standard_value",
            Self::StandardFailure => "standard_failure",
            Self::Application => "application",
            Self::Locales => "locales",
            Self::AuthenticationStrategy => "authentication_strategy",
            Self::CredentialSlot => "credential_slot",
            Self::CredentialValidation => "credential_validation",
            Self::CredentialClaimMapping => "credential_claim_mapping",
            Self::PrincipalResolution => "principal_resolution",
            Self::PrincipalMapping => "principal_mapping",
            Self::Principal => "principal",
            Self::PrincipalVariant => "principal_variant",
            Self::Configuration => "configuration",
            Self::ConfigurationField => "configuration_field",
            Self::Type => "type",
            Self::Enum => "enum",
            Self::EnumVariant => "enum_variant",
            Self::Entity => "entity",
            Self::EntityReference => "entity_reference",
            Self::Value => "object",
            Self::Input => "input",
            Self::Output => "output",
            Self::Field => "field",
            Self::Relationship => "relationship",
            Self::PersistenceConstraint => "persistence_constraint",
            Self::Failure => "failure",
            Self::Function => "function",
            Self::Action => "action",
            Self::Query => "query",
            Self::Fixture => "fixture",
            Self::Test => "test",
            Self::Route => "route",
        }
    }

    const fn is_type(self) -> bool {
        matches!(
            self,
            Self::PreludeType
                | Self::Type
                | Self::Enum
                | Self::Principal
                | Self::PrincipalVariant
                | Self::Entity
                | Self::EntityReference
                | Self::Value
                | Self::Input
                | Self::Output
                | Self::Field
        )
    }

    const fn user_name(self) -> &'static str {
        match self {
            Self::PreludeType | Self::Type => "type",
            Self::StandardNamespace => "standard-library namespace",
            Self::StandardFunction => "standard-library function",
            Self::StandardValue => "standard-library value",
            Self::StandardFailure => "predefined category",
            Self::Application => "application",
            Self::Locales => "locale declaration",
            Self::AuthenticationStrategy => "authentication strategy",
            Self::CredentialSlot => "credential slot",
            Self::CredentialValidation => "credential validation",
            Self::CredentialClaimMapping => "credential claim mapping",
            Self::PrincipalResolution => "principal resolution",
            Self::PrincipalMapping => "principal mapping",
            Self::Principal => "principal declaration",
            Self::PrincipalVariant => "principal variant",
            Self::Configuration => "configuration",
            Self::ConfigurationField => "configuration field",
            Self::Enum => "enum",
            Self::EnumVariant => "enum variant",
            Self::Entity => "entity",
            Self::EntityReference => "entity reference",
            Self::Value => "object type",
            Self::Input => "input",
            Self::Output => "output",
            Self::Field => "field",
            Self::Relationship => "relationship",
            Self::PersistenceConstraint => "persistence constraint",
            Self::Failure => "failure",
            Self::Function => "function",
            Self::Action => "action",
            Self::Query => "query",
            Self::Fixture => "test fixture",
            Self::Test => "test",
            Self::Route => "route",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticNode {
    pub id: NodeId,
    pub kind: NodeKind,
    pub name: String,
    pub source: String,
    pub range: TextRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RefinementEdge {
    pub refined: NodeId,
    pub parent: NodeId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallEdge {
    pub caller: NodeId,
    pub callee: NodeId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticationResolutionEdge {
    pub resolution: NodeId,
    pub authority: NodeId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticGraph {
    pub modules: Vec<SemanticModule>,
    pub nodes: Vec<SemanticNode>,
    pub refinements: Vec<RefinementEdge>,
    /// Named contracts that include absence, including inherited field references.
    pub nullable_types: BTreeSet<String>,
    pub calls: Vec<CallEdge>,
    pub authentication_resolutions: Vec<AuthenticationResolutionEdge>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticModule {
    pub name: String,
    pub source: String,
    pub imports: Vec<String>,
    pub exports: Vec<String>,
}

impl SemanticGraph {
    pub fn node(&self, name: &str) -> Option<&SemanticNode> {
        self.nodes.iter().find(|node| node.name == name)
    }

    pub fn manifest(&self) -> SemanticManifest<'_> {
        SemanticManifest { graph: self }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SemanticManifest<'graph> {
    graph: &'graph SemanticGraph,
}

impl SemanticManifest<'_> {
    pub fn to_json(self) -> String {
        let modules = self
            .graph
            .modules
            .iter()
            .map(|module| {
                format!(
                    "{{\"name\":\"{}\",\"source\":\"{}\",\"imports\":{},\"exports\":{}}}",
                    escape_json(&module.name),
                    escape_json(&module.source),
                    json_strings(&module.imports),
                    json_strings(&module.exports)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let nodes = self
            .graph
            .nodes
            .iter()
            .map(|node| {
                format!(
                    "{{\"id\":{},\"kind\":\"{}\",\"name\":\"{}\",\"source\":\"{}\",\"start\":{},\"end\":{}}}",
                    node.id.0,
                    node.kind.as_str(),
                    escape_json(&node.name),
                    escape_json(&node.source),
                    node.range.start,
                    node.range.end
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let refinements = self
            .graph
            .refinements
            .iter()
            .map(|edge| {
                format!(
                    "{{\"refined\":\"{}\",\"parent\":\"{}\"}}",
                    escape_json(&self.graph.nodes[edge.refined.0 as usize].name),
                    escape_json(&self.graph.nodes[edge.parent.0 as usize].name)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let calls = self
            .graph
            .calls
            .iter()
            .map(|edge| {
                format!(
                    "{{\"caller\":\"{}\",\"callee\":\"{}\"}}",
                    escape_json(&self.graph.nodes[edge.caller.0 as usize].name),
                    escape_json(&self.graph.nodes[edge.callee.0 as usize].name)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let authentication_resolutions = self
            .graph
            .authentication_resolutions
            .iter()
            .map(|edge| {
                format!(
                    "{{\"resolution\":\"{}\",\"authority\":\"{}\"}}",
                    escape_json(&self.graph.nodes[edge.resolution.0 as usize].name),
                    escape_json(&self.graph.nodes[edge.authority.0 as usize].name)
                )
            })
            .collect::<Vec<_>>()
            .join(",");

        format!(
            "{{\"schema_version\":1,\"phase\":\"semantic\",\"modules\":[{modules}],\"nodes\":[{nodes}],\"refinements\":[{refinements}],\"calls\":[{calls}],\"authentication_resolutions\":[{authentication_resolutions}]}}"
        )
    }
}

pub fn checked_manifest_json(
    graph: &SemanticGraph,
    typing: &TypeCheckResult,
    failures: &FailureCheckResult,
) -> String {
    let mut manifest =
        graph
            .manifest()
            .to_json()
            .replacen("\"phase\":\"semantic\"", "\"phase\":\"checked\"", 1);
    manifest.pop();

    let mut expressions = typing.expressions.clone();
    expressions.sort_by(|left, right| {
        (
            &left.source,
            left.range.start,
            left.range.end,
            &left.type_name,
        )
            .cmp(&(
                &right.source,
                right.range.start,
                right.range.end,
                &right.type_name,
            ))
    });
    let expressions = expressions
        .iter()
        .map(|expression| {
            format!(
                "{{\"source\":\"{}\",\"start\":{},\"end\":{},\"type\":\"{}\"}}",
                escape_json(&expression.source),
                expression.range.start,
                expression.range.end,
                escape_json(&expression.type_name)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let mut clock_reads = typing.clock_reads.clone();
    clock_reads.sort_by_key(|read| (read.source.clone(), read.range.start, read.range.end));
    clock_reads.dedup();
    let clock_reads = clock_reads
        .iter()
        .map(|read| {
            format!(
                "{{\"source\":\"{}\",\"start\":{},\"end\":{}}}",
                escape_json(&read.source),
                read.range.start,
                read.range.end
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let contracts = failures
        .contracts
        .iter()
        .map(|contract| {
            format!(
                "{{\"name\":\"{}\",\"kind\":\"{}\",\"http_status\":{},\"code\":\"{}\",\"message\":{},\"public_fields\":{},\"internal_fields\":{}}}",
                escape_json(&contract.name),
                escape_json(&contract.kind),
                contract.http_status,
                escape_json(&contract.code),
                contract
                    .message
                    .as_ref()
                    .map(|message| format!("\"{}\"", escape_json(message)))
                    .unwrap_or_else(|| "null".to_owned()),
                json_strings(&contract.public_fields),
                json_strings(&contract.internal_fields)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let route_failures = failures
        .routes
        .iter()
        .map(|route| {
            format!(
                "{{\"route\":\"{}\",\"failure\":\"{}\",\"kind\":\"{}\",\"http_status\":{},\"derived\":{}}}",
                escape_json(&route.route),
                escape_json(&route.failure),
                escape_json(&route.kind),
                route.http_status,
                route.derived
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let callable_outcomes = failures
        .callables
        .iter()
        .map(|callable| {
            format!(
                "{{\"callable\":\"{}\",\"failures\":{},\"may_suspend\":{},\"completion\":\"before_caller_continues\"}}",
                escape_json(&callable.callable),
                json_strings(&callable.failures),
                callable.may_suspend
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    manifest.push_str(&format!(
        ",\"expression_types\":[{expressions}],\"clock_reads\":[{clock_reads}],\"failure_contracts\":[{contracts}],\"callable_outcomes\":[{callable_outcomes}],\"route_failures\":[{route_failures}]}}"
    ));
    manifest
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScaffoldManifest {
    pub schema_version: u32,
    pub phase: &'static str,
    pub source_files: Vec<String>,
}

impl ScaffoldManifest {
    pub fn to_json(&self) -> String {
        let sources = self
            .source_files
            .iter()
            .map(|source| format!("\"{}\"", escape_json(source)))
            .collect::<Vec<_>>()
            .join(",");

        format!(
            "{{\"schema_version\":{},\"phase\":\"{}\",\"source_files\":[{}]}}",
            self.schema_version, self.phase, sources
        )
    }
}

#[derive(Clone, Debug)]
struct PendingNode {
    kind: NodeKind,
    name: String,
    source: String,
    range: TextRange,
}

#[derive(Clone, Copy, Debug)]
enum ReferenceKind {
    Type,
    Principal,
    PrincipalVariant,
    AuthenticationField,
    Failure,
    StandardFailure,
    Fixture,
}

impl ReferenceKind {
    const fn user_name(self) -> &'static str {
        match self {
            Self::Type => "type",
            Self::Principal => "principal declaration",
            Self::PrincipalVariant => "principal variant",
            Self::AuthenticationField => "authentication authority or principal field",
            Self::Failure => "declared failure",
            Self::StandardFailure => "predefined category",
            Self::Fixture => "test fixture",
        }
    }

    const fn examples(self) -> Option<&'static str> {
        match self {
            Self::StandardFailure => {
                Some("`InvalidValue`, `NotFound`, `Conflict`, or `Unavailable`")
            }
            Self::Type
            | Self::Principal
            | Self::PrincipalVariant
            | Self::AuthenticationField
            | Self::Failure => None,
            Self::Fixture => None,
        }
    }

    const fn accepts(self, kind: NodeKind) -> bool {
        match self {
            Self::Type => kind.is_type(),
            Self::Principal => matches!(kind, NodeKind::Principal),
            Self::PrincipalVariant => matches!(kind, NodeKind::PrincipalVariant),
            Self::AuthenticationField => matches!(kind, NodeKind::Field),
            Self::Failure => matches!(kind, NodeKind::Failure | NodeKind::StandardFailure),
            Self::StandardFailure => matches!(kind, NodeKind::StandardFailure),
            Self::Fixture => matches!(kind, NodeKind::Fixture),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum ReferenceUsage {
    ApplicationPrincipal,
    AuthenticationPrincipal,
    AuthenticationMapping,
    AuthenticationAuthority,
    TypeDefinition,
    LocalAnnotation,
    Field,
    Relationship,
    InverseRelationship,
    FailureKind,
    FailsEntry,
    Parameter,
    ReturnValue,
    RouteInput,
    RouteOutput,
    ConfigurationField,
    TestFixture,
}

impl ReferenceUsage {
    const fn user_name(self) -> &'static str {
        match self {
            Self::ApplicationPrincipal => "application authentication principal",
            Self::AuthenticationPrincipal => "authentication strategy principal",
            Self::AuthenticationMapping => "authentication mapping",
            Self::AuthenticationAuthority => "authentication authority lookup",
            Self::TypeDefinition => "type definition",
            Self::LocalAnnotation => "local type annotation",
            Self::Field => "field",
            Self::Relationship => "relationship",
            Self::InverseRelationship => "inverse relationship",
            Self::FailureKind => "failure's `kind`",
            Self::FailsEntry => "`fails` entry",
            Self::Parameter => "parameter",
            Self::ReturnValue => "return value",
            Self::RouteInput => "route input",
            Self::RouteOutput => "route output",
            Self::ConfigurationField => "configuration field",
            Self::TestFixture => "test fixture",
        }
    }
}

#[derive(Clone, Debug)]
struct PendingReference {
    refined: Option<String>,
    target: String,
    expected: ReferenceKind,
    usage: ReferenceUsage,
    source: String,
    range: TextRange,
}

#[derive(Clone, Debug)]
struct PendingCall {
    caller: String,
    callee: String,
    source: String,
    range: TextRange,
}

#[derive(Clone, Debug)]
struct PendingAuthenticationResolution {
    resolution: String,
    authority: String,
}

pub fn build_semantic_graph(files: &[ParsedSyntax]) -> SemanticGraph {
    let mut builder = GraphBuilder::default();
    builder.add_prelude();
    builder.validate_naming(files);
    builder.configure_modules(files);
    for file in files {
        builder.collect_file(file);
    }
    builder.resolve_nullability();
    builder.validate_storage_nullability(files);
    builder.validate_relationships(files);
    builder.finish()
}

#[derive(Default)]
struct GraphBuilder {
    nodes: BTreeMap<String, PendingNode>,
    modules: Vec<SemanticModule>,
    module_scopes: BTreeMap<String, ModuleScope>,
    references: Vec<PendingReference>,
    nullable_types: BTreeSet<String>,
    nullable_references: Vec<(String, String, TextRange)>,
    calls: Vec<PendingCall>,
    authentication_resolutions: Vec<PendingAuthenticationResolution>,
    diagnostics: Vec<Diagnostic>,
    application: Option<String>,
    locales: bool,
    principal: Option<String>,
    configuration: Option<String>,
    configuration_bindings: BTreeMap<String, (String, TextRange)>,
    authentication_strategies: BTreeSet<String>,
    credential_slots: BTreeMap<String, (String, TextRange)>,
    authentication_principals: Vec<(String, String, TextRange)>,
}

#[derive(Clone, Debug, Default)]
struct ModuleScope {
    imported_names: BTreeSet<String>,
}

impl GraphBuilder {
    fn validate_naming(&mut self, files: &[ParsedSyntax]) {
        for file in files {
            if let Some(module) = &file.file.module {
                for segment in &module.path {
                    self.require_name_case(segment, NameCase::LowerSnake, &file.source_name);
                }
            }
            for declaration in &file.file.declarations {
                match declaration {
                    Declaration::Application(value) => {
                        self.require_name_case(
                            &value.name,
                            NameCase::UpperCamel,
                            &file.source_name,
                        );
                    }
                    Declaration::AuthenticationStrategy(value) => {
                        self.require_name_case(
                            &value.name,
                            NameCase::LowerSnake,
                            &file.source_name,
                        );
                        for validator in &value.validators {
                            self.require_name_case(
                                &validator.name,
                                NameCase::LowerSnake,
                                &file.source_name,
                            );
                        }
                        if let jadpo_syntax::CredentialLocation::Bearer(slot) =
                            &value.transport.location
                        {
                            self.require_name_case(slot, NameCase::LowerSnake, &file.source_name);
                        }
                        for mapping in value.claims.iter().chain(
                            value
                                .resolutions
                                .iter()
                                .flat_map(|resolution| &resolution.mappings),
                        ) {
                            self.require_name_case(
                                &mapping.source,
                                NameCase::LowerSnake,
                                &file.source_name,
                            );
                        }
                    }
                    Declaration::Principal(value) => {
                        self.require_name_case(
                            &value.name,
                            NameCase::UpperCamel,
                            &file.source_name,
                        );
                        for variant in &value.variants {
                            self.require_name_case(
                                &variant.name,
                                NameCase::LowerSnake,
                                &file.source_name,
                            );
                            self.require_field_names(&variant.fields, &file.source_name);
                        }
                    }
                    Declaration::Config(value) => {
                        self.require_name_case(
                            &value.name,
                            NameCase::UpperCamel,
                            &file.source_name,
                        );
                        for field in &value.fields {
                            self.require_name_case(
                                &field.name,
                                NameCase::LowerSnake,
                                &file.source_name,
                            );
                        }
                    }
                    Declaration::Type(value) => {
                        if !value.name.text.contains('.')
                            && !value.name.text.starts_with("__jadpo_")
                        {
                            self.require_name_case(
                                &value.name,
                                NameCase::UpperCamel,
                                &file.source_name,
                            );
                        }
                    }
                    Declaration::Enum(value) => {
                        self.require_name_case(
                            &value.name,
                            NameCase::UpperCamel,
                            &file.source_name,
                        );
                        for variant in &value.variants {
                            self.require_name_case(
                                &variant.name,
                                NameCase::LowerSnake,
                                &file.source_name,
                            );
                            self.require_field_names(&variant.fields, &file.source_name);
                        }
                    }
                    Declaration::Record(value) => {
                        if !value.name.text.contains('.')
                            && !value.name.text.starts_with("__jadpo_")
                        {
                            self.require_name_case(
                                &value.name,
                                NameCase::UpperCamel,
                                &file.source_name,
                            );
                        }
                        self.require_field_names(&value.fields, &file.source_name);
                        for inverse in &value.inverses {
                            self.require_name_case(
                                &inverse.name,
                                NameCase::LowerSnake,
                                &file.source_name,
                            );
                        }
                        for constraint in &value.persistence_constraints {
                            self.require_name_case(
                                &constraint.name,
                                NameCase::LowerSnake,
                                &file.source_name,
                            );
                        }
                        if let Some(dossier) = &value.dossier {
                            if let Some(persistence) = &dossier.persistence {
                                self.require_name_case(
                                    &persistence.store,
                                    NameCase::LowerSnake,
                                    &file.source_name,
                                );
                            }
                            for representation in &dossier.representations {
                                self.require_name_case(
                                    &representation.name,
                                    NameCase::LowerSnake,
                                    &file.source_name,
                                );
                                self.require_name_case(
                                    &representation.store,
                                    NameCase::LowerSnake,
                                    &file.source_name,
                                );
                                self.require_name_case(
                                    &representation.from,
                                    NameCase::LowerSnake,
                                    &file.source_name,
                                );
                                if let Some(strategy) = &representation.strategy {
                                    self.require_name_case(
                                        strategy,
                                        NameCase::LowerSnake,
                                        &file.source_name,
                                    );
                                }
                            }
                        }
                    }
                    Declaration::Failure(value) => {
                        self.require_name_case(
                            &value.name,
                            NameCase::UpperCamel,
                            &file.source_name,
                        );
                        self.require_field_names(&value.public_fields, &file.source_name);
                        self.require_field_names(&value.internal_fields, &file.source_name);
                    }
                    Declaration::Callable(value) => {
                        let operation = value
                            .name
                            .text
                            .rsplit('.')
                            .next()
                            .unwrap_or(&value.name.text);
                        let operation_name = jadpo_syntax::Name {
                            text: operation.to_owned(),
                            range: value.name.range,
                        };
                        self.require_name_case(
                            &operation_name,
                            NameCase::LowerSnake,
                            &file.source_name,
                        );
                        if standard_operation_namespace(operation).is_some() {
                            self.diagnostics.push(with_span(
                                Diagnostic::error("SEM_STANDARD_OPERATION_RESERVED")
                                    .with_fact(DiagnosticFact::Name(operation.to_owned())),
                                &file.source_name,
                                value.name.range,
                            ));
                        }
                        for parameter in &value.parameters {
                            self.require_name_case(
                                &parameter.name,
                                NameCase::LowerSnake,
                                &file.source_name,
                            );
                        }
                        self.require_block_names(&value.body, &file.source_name);
                    }
                    Declaration::Fixture(value) => {
                        self.require_name_case(
                            &value.name,
                            NameCase::LowerSnake,
                            &file.source_name,
                        );
                        if let Some(clock) = &value.clock {
                            self.require_expression_names(clock, &file.source_name);
                        }
                        if let Some(configuration) = &value.configuration {
                            for item in configuration {
                                self.require_name_case(
                                    &item.name,
                                    NameCase::LowerSnake,
                                    &file.source_name,
                                );
                                self.require_expression_names(&item.value, &file.source_name);
                            }
                        }
                    }
                    Declaration::Test(value) => {
                        self.require_block_names(&value.body, &file.source_name);
                    }
                    Declaration::Route(value) => {
                        self.require_field_names(&value.path_fields, &file.source_name);
                        if let Some(run) = &value.run {
                            self.require_expression_names(
                                &Expression::Invocation(run.clone()),
                                &file.source_name,
                            );
                        }
                        if let Some(action) = &value.inline_action {
                            self.require_block_names(&action.body, &file.source_name);
                        }
                    }
                    Declaration::Locales(_) => {}
                }
                if let Some(name) = declaration_name(declaration) {
                    if is_standard_namespace(&name.text) {
                        self.diagnostics.push(with_span(
                            Diagnostic::error("SEM_STANDARD_NAMESPACE_RESERVED")
                                .with_fact(DiagnosticFact::Name(name.text.clone())),
                            &file.source_name,
                            name.range,
                        ));
                    }
                }
            }
        }
    }

    fn require_field_names(&mut self, fields: &[jadpo_syntax::FieldDeclaration], source: &str) {
        for field in fields {
            self.require_name_case(&field.name, NameCase::LowerSnake, source);
            if let Some(reference) = &field.reference {
                if let Some(relationship) = &reference.relationship {
                    self.require_name_case(relationship, NameCase::LowerSnake, source);
                }
            }
        }
    }

    fn require_block_names(&mut self, block: &Block, source: &str) {
        for statement in &block.statements {
            match statement {
                Statement::Binding(statement) => {
                    self.require_name_case(&statement.name, NameCase::LowerSnake, source);
                    self.require_expression_names(&statement.value, source);
                }
                Statement::Assignment(statement) => {
                    self.require_expression_names(&statement.value, source);
                }
                Statement::Return(statement) => {
                    self.require_expression_names(&statement.value, source);
                }
                Statement::Reject(statement) => {
                    for field in &statement.values {
                        self.require_expression_names(&field.value, source);
                    }
                }
                Statement::If(statement) => {
                    self.require_expression_names(&statement.condition, source);
                    self.require_block_names(&statement.then_block, source);
                    if let Some(block) = &statement.else_block {
                        self.require_block_names(block, source);
                    }
                }
                Statement::Match(statement) => {
                    self.require_expression_names(&statement.subject, source);
                    for arm in &statement.arms {
                        match &arm.pattern {
                            jadpo_syntax::MatchPattern::Variant(pattern) => {
                                for binding in &pattern.bindings {
                                    self.require_name_case(binding, NameCase::LowerSnake, source);
                                }
                            }
                            jadpo_syntax::MatchPattern::OptionalSome(pattern) => {
                                self.require_name_case(
                                    &pattern.binding,
                                    NameCase::LowerSnake,
                                    source,
                                );
                            }
                            _ => {}
                        }
                        self.require_block_names(&arm.body, source);
                    }
                }
                Statement::Assert(statement) => {
                    self.require_expression_names(&statement.condition, source);
                }
                Statement::AdvanceClock(statement) => {
                    self.require_expression_names(&statement.duration, source);
                }
                Statement::Unsupported(_) => {}
            }
        }
    }

    fn require_expression_names(&mut self, expression: &Expression, source: &str) {
        match expression {
            Expression::Literal(_) | Expression::Name(_) | Expression::Missing(_) => {}
            Expression::Invocation(value) => {
                for argument in &value.arguments {
                    self.require_expression_names(argument, source);
                }
                for argument in &value.named_arguments {
                    self.require_expression_names(&argument.value, source);
                }
            }
            Expression::TestCall(value) => {
                for argument in &value.invocation.arguments {
                    self.require_expression_names(argument, source);
                }
                for argument in &value.invocation.named_arguments {
                    self.require_expression_names(&argument.value, source);
                }
            }
            Expression::Object(value) => {
                for field in &value.fields {
                    self.require_expression_names(&field.value, source);
                }
            }
            Expression::Construction(value) => {
                for field in &value.fields {
                    self.require_expression_names(&field.value, source);
                }
            }
            Expression::Create(value) => {
                for field in &value.fields {
                    self.require_expression_names(&field.value, source);
                }
                for conflict in &value.conflicts {
                    for field in &conflict.rejection.values {
                        self.require_expression_names(&field.value, source);
                    }
                }
            }
            Expression::Query(value) => {
                self.require_expression_names(&value.value, source);
                if let Some(pagination) = &value.pagination {
                    self.require_expression_names(&pagination.limit, source);
                    self.require_expression_names(&pagination.offset, source);
                }
                for include in &value.includes {
                    self.require_expression_names(&include.pagination.limit, source);
                    self.require_expression_names(&include.pagination.offset, source);
                }
                if let Some(missing) = &value.missing {
                    for field in &missing.values {
                        self.require_expression_names(&field.value, source);
                    }
                }
            }
            Expression::Update(value) => {
                self.require_expression_names(&value.value, source);
                for field in &value.changes {
                    self.require_expression_names(&field.value, source);
                }
                for change in &value.conditional_changes {
                    self.require_expression_names(&change.change.value, source);
                }
                for rejection in value
                    .empty
                    .iter()
                    .chain(std::iter::once(&value.missing))
                    .chain(value.conflicts.iter().map(|conflict| &conflict.rejection))
                {
                    for field in &rejection.values {
                        self.require_expression_names(&field.value, source);
                    }
                }
            }
            Expression::Delete(value) => {
                self.require_expression_names(&value.value, source);
                for rejection in std::iter::once(&value.missing)
                    .chain(value.conflicts.iter().map(|conflict| &conflict.rejection))
                {
                    for field in &rejection.values {
                        self.require_expression_names(&field.value, source);
                    }
                }
            }
            Expression::Attempt(value) => self.require_expression_names(&value.value, source),
            Expression::OutcomeMatch(value) => {
                self.require_expression_names(&value.subject, source);
                for arm in &value.arms {
                    if let jadpo_syntax::OutcomeMatchPattern::Success(binding) = &arm.pattern {
                        self.require_name_case(binding, NameCase::LowerSnake, source);
                    }
                    match &arm.body {
                        jadpo_syntax::OutcomeMatchArmBody::Value(value) => {
                            self.require_expression_names(value, source)
                        }
                        jadpo_syntax::OutcomeMatchArmBody::Reject(value) => {
                            for field in &value.values {
                                self.require_expression_names(&field.value, source);
                            }
                        }
                        jadpo_syntax::OutcomeMatchArmBody::Propagate(_) => {}
                    }
                }
            }
            Expression::Unary(value) => self.require_expression_names(&value.value, source),
            Expression::Binary(value) => {
                self.require_expression_names(&value.left, source);
                self.require_expression_names(&value.right, source);
            }
            Expression::Grouped(value) => self.require_expression_names(&value.value, source),
        }
    }

    fn require_name_case(&mut self, name: &jadpo_syntax::Name, expected: NameCase, source: &str) {
        if expected.accepts(&name.text) {
            return;
        }
        let diagnostic = Diagnostic::error("SEM_NAME_CASE")
            .with_fact(DiagnosticFact::Name(name.text.clone()))
            .with_fact(DiagnosticFact::Expected(expected.description().to_owned()))
            .with_fact(DiagnosticFact::SuggestedName(expected.convert(&name.text)));
        self.diagnostics
            .push(with_span(diagnostic, source, name.range));
    }

    fn add_prelude(&mut self) {
        for name in [
            "Bool",
            "Bytes",
            "Decimal",
            "Duration",
            "Int",
            "List",
            "Map",
            "Object",
            "Set",
            "Text",
            "Unit",
            "Uuid",
            "Instant",
            "CalendarDate",
            "Time",
            "Zone",
            "Locale",
            "PresentationText",
            "InstantRange",
            "LocalOverlap",
            "LocalGap",
            "InvalidDay",
            "Weekday",
            "TimeFormat",
            "FriendlyTimeFormat",
        ] {
            self.add_node(PendingNode {
                kind: NodeKind::PreludeType,
                name: name.to_owned(),
                source: "<prelude>".to_owned(),
                range: TextRange::new(0, 0),
            });
        }
        for (owner, variants) in [
            ("LocalOverlap", &["reject", "earlier", "later"][..]),
            (
                "LocalGap",
                &["reject", "shift_forward", "shift_backward"][..],
            ),
            ("InvalidDay", &["reject", "last_valid_day"][..]),
            (
                "Weekday",
                &[
                    "monday",
                    "tuesday",
                    "wednesday",
                    "thursday",
                    "friday",
                    "saturday",
                    "sunday",
                ][..],
            ),
            (
                "TimeFormat",
                &[
                    "date_full",
                    "date_long",
                    "date_medium",
                    "date_short",
                    "time_full",
                    "time_long",
                    "time_medium",
                    "time_short",
                    "date_time_full",
                    "date_time_long",
                    "date_time_medium",
                    "date_time_short",
                ][..],
            ),
            ("FriendlyTimeFormat", &["conversational"][..]),
        ] {
            for variant in variants {
                self.add_node(PendingNode {
                    kind: NodeKind::StandardValue,
                    name: format!("{owner}.{variant}"),
                    source: "<prelude>".to_owned(),
                    range: TextRange::new(0, 0),
                });
            }
        }
        for zone in include_str!("../../../data/iana-zones-2026c.txt").lines() {
            self.add_node(PendingNode {
                kind: NodeKind::StandardValue,
                name: format!("Zone.{}", standard_variant_name(zone)),
                source: "<prelude>".to_owned(),
                range: TextRange::new(0, 0),
            });
        }
        self.add_node(PendingNode {
            kind: NodeKind::StandardNamespace,
            name: "temporal".to_owned(),
            source: "<prelude>".to_owned(),
            range: TextRange::new(0, 0),
        });
        self.add_node(PendingNode {
            kind: NodeKind::StandardNamespace,
            name: "collection".to_owned(),
            source: "<prelude>".to_owned(),
            range: TextRange::new(0, 0),
        });
        for name in [
            "in_zone",
            "resolve",
            "add_elapsed",
            "between",
            "add_days",
            "add_weeks",
            "add_months",
            "add_years",
            "add_local_days",
            "add_local_weeks",
            "add_local_months",
            "add_local_years",
            "day_bounds",
            "week_bounds",
            "month_bounds",
            "year_bounds",
            "calendar_date",
            "year",
            "month",
            "day",
            "weekday",
            "hour",
            "minute",
            "second",
            "millisecond",
            "offset",
            "zone",
            "same_zone",
            "same_local",
            "format",
            "format_friendly",
        ] {
            self.add_node(PendingNode {
                kind: NodeKind::StandardFunction,
                name: format!("temporal.{name}"),
                source: "<prelude>".to_owned(),
                range: TextRange::new(0, 0),
            });
        }
        for name in ["Email", "Url", "IpAddress"] {
            self.add_node(PendingNode {
                kind: NodeKind::Type,
                name: name.to_owned(),
                source: "<prelude>".to_owned(),
                range: TextRange::new(0, 0),
            });
            self.references.push(PendingReference {
                refined: Some(name.to_owned()),
                target: "Text".to_owned(),
                expected: ReferenceKind::Type,
                usage: ReferenceUsage::TypeDefinition,
                source: "<prelude>".to_owned(),
                range: TextRange::new(0, 0),
            });
        }
        for name in [
            "Conflict",
            "InternalFault",
            "InvalidRequest",
            "InvalidValue",
            "Misconfigured",
            "NotFound",
            "NotPermitted",
            "NotVisible",
            "OutcomeUnknown",
            "PreconditionFailed",
            "RateLimited",
            "Rejected",
            "TimedOut",
            "Unauthenticated",
            "Unavailable",
        ] {
            self.add_node(PendingNode {
                kind: NodeKind::StandardFailure,
                name: name.to_owned(),
                source: "<prelude>".to_owned(),
                range: TextRange::new(0, 0),
            });
        }
    }

    fn configure_modules(&mut self, files: &[ParsedSyntax]) {
        let explicit_modules = files.iter().any(|file| file.file.module.is_some());
        if !explicit_modules {
            for file in files {
                if let Some(export) = file.file.exports.first() {
                    self.diagnostics.push(with_span(
                        Diagnostic::error("MOD_PUBLIC_REQUIRES_MODULE"),
                        &file.source_name,
                        export.range,
                    ));
                }
            }
            return;
        }

        let mut files_by_module = BTreeMap::<String, &ParsedSyntax>::new();
        for file in files {
            let Some(module) = &file.file.module else {
                self.diagnostics.push(with_span(
                    Diagnostic::error("MOD_MODULE_REQUIRED"),
                    &file.source_name,
                    file.file.range,
                ));
                continue;
            };
            let name = name_expression(&module.path);
            if module
                .path
                .first()
                .is_some_and(|segment| is_standard_namespace(&segment.text))
            {
                self.diagnostics.push(with_span(
                    Diagnostic::error("MOD_STANDARD_NAMESPACE_RESERVED")
                        .with_fact(DiagnosticFact::Name(name.clone())),
                    &file.source_name,
                    module.range,
                ));
            }
            if let Some(previous) = files_by_module.get(&name) {
                self.diagnostics.push(with_span(
                    Diagnostic::error("MOD_DUPLICATE_MODULE")
                        .with_note(format!("first module file is {}", previous.source_name)),
                    &file.source_name,
                    module.range,
                ));
            } else {
                files_by_module.insert(name, file);
            }
        }

        let mut dependencies = files_by_module
            .keys()
            .map(|name| (name.clone(), BTreeSet::new()))
            .collect::<BTreeMap<_, _>>();

        for file in files {
            let Some(module) = &file.file.module else {
                continue;
            };
            let module_name = name_expression(&module.path);
            let local_names = file
                .file
                .declarations
                .iter()
                .filter_map(declaration_name)
                .map(|name| name.text.as_str())
                .collect::<BTreeSet<_>>();
            let mut imported_names = BTreeSet::new();
            let mut imports = Vec::new();
            for import in &file.file.imports {
                let target_name = name_expression(&import.module);
                if import
                    .module
                    .first()
                    .is_some_and(|segment| is_standard_namespace(&segment.text))
                {
                    self.diagnostics.push(with_span(
                        Diagnostic::error("MOD_STANDARD_NAMESPACE_IMPORT")
                            .with_fact(DiagnosticFact::Name(target_name)),
                        &file.source_name,
                        import.range,
                    ));
                    continue;
                }
                if target_name == module_name {
                    self.diagnostics.push(with_span(
                        Diagnostic::error("MOD_SELF_IMPORT"),
                        &file.source_name,
                        import.range,
                    ));
                    continue;
                }
                let Some(target) = files_by_module.get(&target_name).copied() else {
                    self.diagnostics.push(with_span(
                        Diagnostic::error("MOD_UNKNOWN_MODULE"),
                        &file.source_name,
                        import.range,
                    ));
                    continue;
                };
                dependencies
                    .entry(module_name.clone())
                    .or_default()
                    .insert(target_name.clone());
                let target_exports = target
                    .file
                    .exports
                    .iter()
                    .map(|name| name.text.as_str())
                    .collect::<BTreeSet<_>>();
                let target_declarations = target
                    .file
                    .declarations
                    .iter()
                    .filter_map(declaration_name)
                    .map(|name| name.text.as_str())
                    .collect::<BTreeSet<_>>();
                for name in &import.names {
                    if local_names.contains(name.text.as_str()) {
                        self.diagnostics.push(with_span(
                            Diagnostic::error("MOD_IMPORT_CONFLICT"),
                            &file.source_name,
                            name.range,
                        ));
                        continue;
                    }
                    if !imported_names.insert(name.text.clone()) {
                        self.diagnostics.push(with_span(
                            Diagnostic::error("MOD_DUPLICATE_IMPORT"),
                            &file.source_name,
                            name.range,
                        ));
                        continue;
                    }
                    if !target_exports.contains(name.text.as_str()) {
                        let (code, _message) = if target_declarations.contains(name.text.as_str()) {
                            (
                                "MOD_PRIVATE_IMPORT",
                                format!(
                                    "declaration `{}` is private to module `{target_name}`",
                                    name.text
                                ),
                            )
                        } else {
                            (
                                "MOD_UNKNOWN_EXPORT",
                                format!(
                                    "module `{target_name}` has no exported declaration `{}`",
                                    name.text
                                ),
                            )
                        };
                        self.diagnostics.push(with_span(
                            Diagnostic::error(code),
                            &file.source_name,
                            name.range,
                        ));
                        imported_names.remove(&name.text);
                        continue;
                    }
                    imports.push(format!("{target_name}.{}", name.text));
                }
            }

            let mut exports = file
                .file
                .exports
                .iter()
                .map(|name| name.text.clone())
                .collect::<Vec<_>>();
            exports.sort();
            imports.sort();
            self.modules.push(SemanticModule {
                name: module_name.clone(),
                source: file.source_name.clone(),
                imports,
                exports,
            });
            self.module_scopes
                .insert(file.source_name.clone(), ModuleScope { imported_names });
        }

        let mut emitted = BTreeSet::new();
        loop {
            let ready = dependencies
                .iter()
                .filter(|(name, required)| {
                    !emitted.contains(*name)
                        && required.iter().all(|module| emitted.contains(module))
                })
                .map(|(name, _)| name.clone())
                .collect::<Vec<_>>();
            if ready.is_empty() {
                break;
            }
            emitted.extend(ready);
        }
        if emitted.len() != dependencies.len() {
            let cyclic = dependencies
                .keys()
                .filter(|name| !emitted.contains(*name))
                .cloned()
                .collect::<BTreeSet<_>>();
            for file in files {
                let Some(module) = &file.file.module else {
                    continue;
                };
                let module_name = name_expression(&module.path);
                if !cyclic.contains(&module_name) {
                    continue;
                }
                if let Some(import) = file
                    .file
                    .imports
                    .iter()
                    .find(|import| cyclic.contains(&name_expression(&import.module)))
                {
                    self.diagnostics.push(with_span(
                        Diagnostic::error("MOD_IMPORT_CYCLE"),
                        &file.source_name,
                        import.range,
                    ));
                }
            }
        }
        self.modules
            .sort_by(|left, right| left.name.cmp(&right.name));
    }

    fn collect_file(&mut self, file: &ParsedSyntax) {
        for declaration in &file.file.declarations {
            match declaration {
                Declaration::Locales(declaration) => {
                    if self.locales {
                        self.diagnostics.push(with_span(
                            Diagnostic::error("SEM_MULTIPLE_LOCALE_DECLARATIONS"),
                            &file.source_name,
                            declaration.range,
                        ));
                        self.add_node(PendingNode {
                            kind: NodeKind::Locales,
                            name: format!("locales#{}", declaration.range.start),
                            source: file.source_name.clone(),
                            range: declaration.range,
                        });
                    } else {
                        self.locales = true;
                        self.add_node(PendingNode {
                            kind: NodeKind::Locales,
                            name: "locales".to_owned(),
                            source: file.source_name.clone(),
                            range: declaration.range,
                        });
                        let mut emitted = BTreeSet::new();
                        for locale in &declaration.supported {
                            let tag = locale.text.trim_matches('"');
                            let name = format!("Locale.{}", standard_variant_name(tag));
                            if emitted.insert(name.clone()) {
                                self.add_node(PendingNode {
                                    kind: NodeKind::StandardValue,
                                    name,
                                    source: file.source_name.clone(),
                                    range: locale.range,
                                });
                            }
                        }
                    }
                }
                Declaration::Application(declaration) => {
                    if self.application.is_some() {
                        self.diagnostics.push(with_span(
                            Diagnostic::error("SEM_MULTIPLE_APPLICATIONS"),
                            &file.source_name,
                            declaration.name.range,
                        ));
                    } else {
                        self.application = Some(declaration.name.text.clone());
                    }
                    self.add_authored_node(
                        NodeKind::Application,
                        &declaration.name.text,
                        &file.source_name,
                        declaration.name.range,
                    );
                    let principal_reference = &declaration.authentication.principal;
                    if principal_reference.nullable
                        || !principal_reference.arguments.is_empty()
                        || principal_reference.path.len() != 1
                    {
                        self.diagnostics.push(with_span(
                            Diagnostic::error("SEM_APPLICATION_PRINCIPAL_SHAPE"),
                            &file.source_name,
                            principal_reference.range,
                        ));
                    } else {
                        self.references.push(PendingReference {
                            refined: None,
                            target: name_expression(&principal_reference.path),
                            expected: ReferenceKind::Principal,
                            usage: ReferenceUsage::ApplicationPrincipal,
                            source: file.source_name.clone(),
                            range: principal_reference.range,
                        });
                    }
                    let revocation = &declaration.authentication.revocation;
                    match (revocation.mode, revocation.maximum_delay.as_ref()) {
                        (jadpo_syntax::RevocationMode::Bounded, None) => {
                            self.diagnostics.push(with_span(
                                Diagnostic::error("SEM_REVOCATION_DELAY_REQUIRED"),
                                &file.source_name,
                                revocation.range,
                            ));
                        }
                        (jadpo_syntax::RevocationMode::Immediate, Some(delay)) => {
                            self.diagnostics.push(with_span(
                                Diagnostic::error("SEM_REVOCATION_DELAY_FORBIDDEN"),
                                &file.source_name,
                                delay.range,
                            ));
                        }
                        (_, Some(delay)) if !positive_duration(&delay.text) => {
                            self.diagnostics.push(with_span(
                                Diagnostic::error("SEM_REVOCATION_DELAY_INVALID"),
                                &file.source_name,
                                delay.range,
                            ));
                        }
                        _ => {}
                    }
                }
                Declaration::AuthenticationStrategy(declaration) => {
                    if !self
                        .authentication_strategies
                        .insert(declaration.name.text.clone())
                    {
                        self.diagnostics.push(with_span(
                            Diagnostic::error("SEM_AUTH_STRATEGY_DUPLICATE"),
                            &file.source_name,
                            declaration.name.range,
                        ));
                        continue;
                    }
                    let strategy_name = format!("authentication.{}", declaration.name.text);
                    self.add_authored_node(
                        NodeKind::AuthenticationStrategy,
                        &strategy_name,
                        &file.source_name,
                        declaration.name.range,
                    );

                    if declaration.validators.is_empty() {
                        self.diagnostics.push(with_span(
                            Diagnostic::error("SEM_AUTH_VALIDATOR_REQUIRED"),
                            &file.source_name,
                            declaration.range,
                        ));
                    }
                    let mut validators = BTreeSet::new();
                    for validator in &declaration.validators {
                        if !validators.insert(validator.name.text.as_str()) {
                            self.diagnostics.push(with_span(
                                Diagnostic::error("SEM_AUTH_VALIDATOR_DUPLICATE"),
                                &file.source_name,
                                validator.name.range,
                            ));
                            continue;
                        }
                        if matches!(
                            validator.mode.text.as_str(),
                            "signed" | "opaque" | "api_key" | "jwt"
                        ) {
                            self.add_authored_node(
                                NodeKind::CredentialValidation,
                                &format!(
                                    "{strategy_name}.validator.{}.{}",
                                    validator.name.text, validator.mode.text
                                ),
                                &file.source_name,
                                validator.range,
                            );
                        } else {
                            self.diagnostics.push(with_span(
                                Diagnostic::error("SEM_AUTH_VALIDATION_MODE_UNKNOWN"),
                                &file.source_name,
                                validator.mode.range,
                            ));
                        }

                        if matches!(validator.principal.text.as_str(), "user" | "service") {
                            self.authentication_principals.push((
                                validator.principal.text.clone(),
                                file.source_name.clone(),
                                validator.principal.range,
                            ));
                        } else {
                            self.diagnostics.push(with_span(
                                Diagnostic::error("SEM_AUTH_PRINCIPAL_VARIANT_UNKNOWN"),
                                &file.source_name,
                                validator.principal.range,
                            ));
                        }
                    }

                    let mut claim_targets = BTreeSet::new();
                    for mapping in &declaration.claims {
                        let target = name_expression(&mapping.target.path);
                        if !claim_targets.insert(target.clone()) {
                            self.diagnostics.push(with_span(
                                Diagnostic::error("SEM_AUTH_MAPPING_DUPLICATE"),
                                &file.source_name,
                                mapping.target.range,
                            ));
                            continue;
                        }
                        if mapping.target.path.len() != 3
                            || !matches!(mapping.target.path[1].text.as_str(), "user" | "service")
                        {
                            self.diagnostics.push(with_span(
                                Diagnostic::error("SEM_AUTH_MAPPING_TARGET_INVALID"),
                                &file.source_name,
                                mapping.target.range,
                            ));
                            continue;
                        }
                        self.add_authored_node(
                            NodeKind::CredentialClaimMapping,
                            &format!("{strategy_name}.claim.{}.{}", mapping.source.text, target),
                            &file.source_name,
                            mapping.range,
                        );
                        self.references.push(PendingReference {
                            refined: None,
                            target,
                            expected: ReferenceKind::AuthenticationField,
                            usage: ReferenceUsage::AuthenticationMapping,
                            source: file.source_name.clone(),
                            range: mapping.target.range,
                        });
                    }

                    let mut resolution_variants = BTreeSet::new();
                    for resolution in &declaration.resolutions {
                        if !matches!(resolution.principal.text.as_str(), "user" | "service") {
                            self.diagnostics.push(with_span(
                                Diagnostic::error("SEM_AUTH_PRINCIPAL_VARIANT_UNKNOWN"),
                                &file.source_name,
                                resolution.principal.range,
                            ));
                            continue;
                        }
                        if !resolution_variants.insert(resolution.principal.text.as_str()) {
                            self.diagnostics.push(with_span(
                                Diagnostic::error("SEM_AUTH_RESOLUTION_DUPLICATE"),
                                &file.source_name,
                                resolution.principal.range,
                            ));
                            continue;
                        }
                        let resolution_name =
                            format!("{strategy_name}.resolution.{}", resolution.principal.text);
                        self.add_authored_node(
                            NodeKind::PrincipalResolution,
                            &resolution_name,
                            &file.source_name,
                            resolution.range,
                        );
                        let authority = name_expression(&resolution.authority.path);
                        if resolution.authority.path.len() != 2 {
                            self.diagnostics.push(with_span(
                                Diagnostic::error("SEM_AUTH_RESOLUTION_AUTHORITY_INVALID"),
                                &file.source_name,
                                resolution.authority.range,
                            ));
                        } else {
                            self.authentication_resolutions
                                .push(PendingAuthenticationResolution {
                                    resolution: resolution_name.clone(),
                                    authority: authority.clone(),
                                });
                            self.references.push(PendingReference {
                                refined: None,
                                target: authority,
                                expected: ReferenceKind::AuthenticationField,
                                usage: ReferenceUsage::AuthenticationAuthority,
                                source: file.source_name.clone(),
                                range: resolution.authority.range,
                            });
                        }
                        if resolution.mappings.is_empty() {
                            self.diagnostics.push(with_span(
                                Diagnostic::error("SEM_AUTH_MAPPING_REQUIRED"),
                                &file.source_name,
                                resolution.range,
                            ));
                        }
                        let authority_root = resolution
                            .authority
                            .path
                            .first()
                            .map(|name| name.text.as_str())
                            .unwrap_or_default();
                        let mut targets = BTreeSet::new();
                        for mapping in &resolution.mappings {
                            let target = name_expression(&mapping.target.path);
                            if !targets.insert(target.clone()) {
                                self.diagnostics.push(with_span(
                                    Diagnostic::error("SEM_AUTH_MAPPING_DUPLICATE"),
                                    &file.source_name,
                                    mapping.target.range,
                                ));
                                continue;
                            }
                            if mapping.target.path.len() != 3
                                || mapping.target.path[1].text != resolution.principal.text
                            {
                                self.diagnostics.push(with_span(
                                    Diagnostic::error("SEM_AUTH_MAPPING_TARGET_INVALID"),
                                    &file.source_name,
                                    mapping.target.range,
                                ));
                                continue;
                            }
                            self.add_authored_node(
                                NodeKind::PrincipalMapping,
                                &format!("{resolution_name}.{}.{}", mapping.source.text, target),
                                &file.source_name,
                                mapping.range,
                            );
                            for (reference_target, range) in [
                                (
                                    format!("{authority_root}.{}", mapping.source.text),
                                    mapping.source.range,
                                ),
                                (target, mapping.target.range),
                            ] {
                                self.references.push(PendingReference {
                                    refined: None,
                                    target: reference_target,
                                    expected: ReferenceKind::AuthenticationField,
                                    usage: ReferenceUsage::AuthenticationMapping,
                                    source: file.source_name.clone(),
                                    range,
                                });
                            }
                        }
                        self.references.push(PendingReference {
                            refined: None,
                            target: resolution.inactive.text.clone(),
                            expected: ReferenceKind::Failure,
                            usage: ReferenceUsage::AuthenticationMapping,
                            source: file.source_name.clone(),
                            range: resolution.inactive.range,
                        });
                    }

                    let (slot_key, slot_name, slot_range) = match &declaration.transport.location {
                        jadpo_syntax::CredentialLocation::Cookie(location) => {
                            let cookie = unquote_string(&location.text);
                            (
                                format!("cookie:{cookie}"),
                                format!("{strategy_name}.cookie.{cookie}"),
                                location.range,
                            )
                        }
                        jadpo_syntax::CredentialLocation::Bearer(location) => {
                            if location.text != "authorization_header" {
                                self.diagnostics.push(with_span(
                                    Diagnostic::error("SEM_AUTH_BEARER_LOCATION_FORBIDDEN"),
                                    &file.source_name,
                                    location.range,
                                ));
                            }
                            (
                                format!("bearer:{}", location.text),
                                format!("{strategy_name}.bearer.{}", location.text),
                                location.range,
                            )
                        }
                    };
                    if self
                        .credential_slots
                        .insert(slot_key, (declaration.name.text.clone(), slot_range))
                        .is_some()
                    {
                        self.diagnostics.push(with_span(
                            Diagnostic::error("SEM_AUTH_CREDENTIAL_SLOT_DUPLICATE"),
                            &file.source_name,
                            slot_range,
                        ));
                    } else {
                        self.add_authored_node(
                            NodeKind::CredentialSlot,
                            &slot_name,
                            &file.source_name,
                            slot_range,
                        );
                    }
                }
                Declaration::Principal(declaration) => {
                    if self.principal.is_some() {
                        self.diagnostics.push(with_span(
                            Diagnostic::error("SEM_MULTIPLE_PRINCIPALS"),
                            &file.source_name,
                            declaration.name.range,
                        ));
                    } else {
                        self.principal = Some(declaration.name.text.clone());
                    }
                    self.add_authored_node(
                        NodeKind::Principal,
                        &declaration.name.text,
                        &file.source_name,
                        declaration.name.range,
                    );
                    let mut variants = BTreeSet::new();
                    for variant in &declaration.variants {
                        let variant_name =
                            format!("{}.{}", declaration.name.text, variant.kind.as_str());
                        if !variants.insert(variant.kind.as_str()) {
                            self.diagnostics.push(with_span(
                                Diagnostic::error("SEM_PRINCIPAL_VARIANT_DUPLICATE"),
                                &file.source_name,
                                variant.name.range,
                            ));
                            continue;
                        }
                        self.add_authored_node(
                            NodeKind::PrincipalVariant,
                            &variant_name,
                            &file.source_name,
                            variant.name.range,
                        );
                        for field in &variant.fields {
                            if !field.persistence.is_empty() || field.reference.is_some() {
                                self.diagnostics.push(with_span(
                                    Diagnostic::error("DATA_MODIFIER_NON_ENTITY"),
                                    &file.source_name,
                                    field.range,
                                ));
                            }
                            let field_name = format!("{variant_name}.{}", field.name.text);
                            self.add_authored_node(
                                NodeKind::Field,
                                &field_name,
                                &file.source_name,
                                field.name.range,
                            );
                            self.add_type_reference(
                                Some(field_name),
                                &field.field_type,
                                &file.source_name,
                                ReferenceUsage::Field,
                            );
                        }
                    }
                    if !variants.contains("user") || !variants.contains("service") {
                        self.diagnostics.push(with_span(
                            Diagnostic::error("SEM_PRINCIPAL_VARIANT_REQUIRED"),
                            &file.source_name,
                            declaration.range,
                        ));
                    }
                }
                Declaration::Config(declaration) => {
                    if self.configuration.is_some() {
                        self.diagnostics.push(with_span(
                            Diagnostic::error("CONFIG_MULTIPLE_DECLARATIONS"),
                            &file.source_name,
                            declaration.name.range,
                        ));
                    } else {
                        self.configuration = Some(declaration.name.text.clone());
                    }
                    self.add_authored_node(
                        NodeKind::Configuration,
                        &declaration.name.text,
                        &file.source_name,
                        declaration.name.range,
                    );
                    for field in &declaration.fields {
                        let field_name = format!("{}.{}", declaration.name.text, field.name.text);
                        self.add_authored_node(
                            NodeKind::ConfigurationField,
                            &field_name,
                            &file.source_name,
                            field.name.range,
                        );
                        self.add_type_reference(
                            Some(field_name),
                            &field.field_type,
                            &file.source_name,
                            ReferenceUsage::ConfigurationField,
                        );
                        let Some(binding) = &field.binding else {
                            self.diagnostics.push(with_span(
                                Diagnostic::error("CONFIG_BINDING_REQUIRED"),
                                &file.source_name,
                                field.range,
                            ));
                            continue;
                        };
                        let binding_name = unquote_string(&binding.text);
                        if binding_name.is_empty() {
                            self.diagnostics.push(with_span(
                                Diagnostic::error("CONFIG_BINDING_EMPTY"),
                                &file.source_name,
                                binding.range,
                            ));
                        } else if !valid_configuration_binding(&binding_name) {
                            self.diagnostics.push(with_span(
                                Diagnostic::error("CONFIG_BINDING_INVALID"),
                                &file.source_name,
                                binding.range,
                            ));
                        } else if self
                            .configuration_bindings
                            .insert(binding_name, (file.source_name.clone(), binding.range))
                            .is_some()
                        {
                            self.diagnostics.push(with_span(
                                Diagnostic::error("CONFIG_DUPLICATE_BINDING"),
                                &file.source_name,
                                binding.range,
                            ));
                        }
                        if field.secret && field.default.is_some() {
                            self.diagnostics.push(with_span(
                                Diagnostic::error("CONFIG_SECRET_DEFAULT"),
                                &file.source_name,
                                field.default.as_ref().expect("default exists").range,
                            ));
                        }
                    }
                }
                Declaration::Type(declaration) => {
                    self.add_authored_node(
                        NodeKind::Type,
                        &declaration.name.text,
                        &file.source_name,
                        declaration.name.range,
                    );
                    self.add_type_reference(
                        Some(declaration.name.text.clone()),
                        &declaration.parent,
                        &file.source_name,
                        ReferenceUsage::TypeDefinition,
                    );
                }
                Declaration::Enum(declaration) => {
                    self.add_authored_node(
                        NodeKind::Enum,
                        &declaration.name.text,
                        &file.source_name,
                        declaration.name.range,
                    );
                    for variant in &declaration.variants {
                        let variant_name =
                            format!("{}.{}", declaration.name.text, variant.name.text);
                        self.add_authored_node(
                            NodeKind::EnumVariant,
                            &variant_name,
                            &file.source_name,
                            variant.name.range,
                        );
                        for field in &variant.fields {
                            if !field.persistence.is_empty() || field.reference.is_some() {
                                let diagnostic = Diagnostic::error("DATA_MODIFIER_NON_ENTITY");
                                self.diagnostics.push(with_span(
                                    diagnostic,
                                    &file.source_name,
                                    field.range,
                                ));
                            }
                            let field_name = format!("{variant_name}.{}", field.name.text);
                            self.add_authored_node(
                                NodeKind::Field,
                                &field_name,
                                &file.source_name,
                                field.name.range,
                            );
                            self.add_type_reference(
                                Some(field_name),
                                &field.field_type,
                                &file.source_name,
                                ReferenceUsage::Field,
                            );
                        }
                    }
                }
                Declaration::Record(declaration) => {
                    let kind = match declaration.kind {
                        RecordKind::Entity => NodeKind::Entity,
                        RecordKind::Value => NodeKind::Value,
                        RecordKind::Input => NodeKind::Input,
                        RecordKind::Output => NodeKind::Output,
                    };
                    self.add_authored_node(
                        kind,
                        &declaration.name.text,
                        &file.source_name,
                        declaration.name.range,
                    );
                    if declaration.kind == RecordKind::Entity {
                        self.add_authored_node(
                            NodeKind::EntityReference,
                            &format!("{}.Ref", declaration.name.text),
                            &file.source_name,
                            declaration.name.range,
                        );
                    }
                    let mut identity_field = None;
                    for field in &declaration.fields {
                        if declaration.kind != RecordKind::Entity
                            && (!field.persistence.is_empty() || field.reference.is_some())
                        {
                            let diagnostic = Diagnostic::error("DATA_MODIFIER_NON_ENTITY");
                            self.diagnostics.push(with_span(
                                diagnostic,
                                &file.source_name,
                                field.range,
                            ));
                        }
                        if field.persistence.contains(&PersistenceModifier::Identity) {
                            if let Some(_previous) = identity_field {
                                let diagnostic = Diagnostic::error("DATA_MULTIPLE_IDENTITIES");
                                self.diagnostics.push(with_span(
                                    diagnostic,
                                    &file.source_name,
                                    field.range,
                                ));
                            } else {
                                identity_field = Some(field.name.text.as_str());
                            }
                        }
                        let field_name = format!("{}.{}", declaration.name.text, field.name.text);
                        self.add_authored_node(
                            NodeKind::Field,
                            &field_name,
                            &file.source_name,
                            field.name.range,
                        );
                        self.add_type_reference(
                            Some(field_name),
                            &field.field_type,
                            &file.source_name,
                            ReferenceUsage::Field,
                        );
                        if let Some(reference) = &field.reference {
                            self.add_type_reference(
                                None,
                                &reference.target,
                                &file.source_name,
                                ReferenceUsage::Relationship,
                            );
                            if let Some(relationship) = &reference.relationship {
                                self.add_authored_node(
                                    NodeKind::Relationship,
                                    &format!("{}.{}", declaration.name.text, relationship.text),
                                    &file.source_name,
                                    relationship.range,
                                );
                            }
                        }
                    }
                    for inverse in &declaration.inverses {
                        let inverse_name =
                            format!("{}.{}", declaration.name.text, inverse.name.text);
                        self.add_authored_node(
                            NodeKind::Relationship,
                            &inverse_name,
                            &file.source_name,
                            inverse.name.range,
                        );
                        self.references.push(PendingReference {
                            refined: None,
                            target: inverse.target.text.clone(),
                            expected: ReferenceKind::Type,
                            usage: ReferenceUsage::InverseRelationship,
                            source: file.source_name.clone(),
                            range: inverse.target.range,
                        });
                        self.add_type_reference(
                            None,
                            &inverse.via,
                            &file.source_name,
                            ReferenceUsage::InverseRelationship,
                        );
                    }
                    let fields = declaration
                        .fields
                        .iter()
                        .map(|field| (field.name.text.as_str(), field))
                        .collect::<BTreeMap<_, _>>();
                    let mut constraint_shapes = BTreeSet::new();
                    for constraint in &declaration.persistence_constraints {
                        self.add_authored_node(
                            NodeKind::PersistenceConstraint,
                            &format!("{}.{}", declaration.name.text, constraint.name.text),
                            &file.source_name,
                            constraint.name.range,
                        );
                        if constraint.fields.len() < 2 {
                            self.diagnostics.push(with_span(
                                Diagnostic::error("DATA_COMPOUND_CONSTRAINT_FIELDS"),
                                &file.source_name,
                                constraint.range,
                            ));
                        }
                        let mut seen_fields = BTreeSet::new();
                        for field_name in &constraint.fields {
                            if !seen_fields.insert(field_name.text.as_str()) {
                                self.diagnostics.push(with_span(
                                    Diagnostic::error("DATA_CONSTRAINT_DUPLICATE_FIELD"),
                                    &file.source_name,
                                    field_name.range,
                                ));
                                continue;
                            }
                            let Some(_field) = fields.get(field_name.text.as_str()) else {
                                self.diagnostics.push(with_span(
                                    Diagnostic::error("DATA_CONSTRAINT_UNKNOWN_FIELD"),
                                    &file.source_name,
                                    field_name.range,
                                ));
                                continue;
                            };
                        }
                        let mut shape = constraint
                            .fields
                            .iter()
                            .map(|field| field.text.as_str())
                            .collect::<Vec<_>>();
                        shape.sort_unstable();
                        if !constraint_shapes.insert(shape) {
                            self.diagnostics.push(with_span(
                                Diagnostic::error("DATA_DUPLICATE_CONSTRAINT_SHAPE"),
                                &file.source_name,
                                constraint.range,
                            ));
                        }
                    }
                }
                Declaration::Failure(declaration) => {
                    self.add_authored_node(
                        NodeKind::Failure,
                        &declaration.name.text,
                        &file.source_name,
                        declaration.name.range,
                    );
                    self.references.push(PendingReference {
                        refined: None,
                        target: declaration.kind.text.clone(),
                        expected: ReferenceKind::StandardFailure,
                        usage: ReferenceUsage::FailureKind,
                        source: file.source_name.clone(),
                        range: declaration.kind.range,
                    });
                    for (scope, fields) in [
                        ("public", &declaration.public_fields),
                        ("internal", &declaration.internal_fields),
                    ] {
                        for field in fields {
                            let field_name =
                                format!("{}.{}.{}", declaration.name.text, scope, field.name.text);
                            self.add_authored_node(
                                NodeKind::Field,
                                &field_name,
                                &file.source_name,
                                field.name.range,
                            );
                            self.add_type_reference(
                                Some(field_name),
                                &field.field_type,
                                &file.source_name,
                                ReferenceUsage::Field,
                            );
                        }
                    }
                }
                Declaration::Callable(declaration) => {
                    let kind = match declaration.kind {
                        CallableKind::Function => NodeKind::Function,
                        CallableKind::Action => NodeKind::Action,
                        CallableKind::Query => NodeKind::Query,
                    };
                    let callable_name = declaration.name.text.clone();
                    self.add_authored_node(
                        kind,
                        &callable_name,
                        &file.source_name,
                        declaration.name.range,
                    );
                    for parameter in &declaration.parameters {
                        self.add_type_reference(
                            None,
                            &parameter.parameter_type,
                            &file.source_name,
                            ReferenceUsage::Parameter,
                        );
                    }
                    self.add_type_reference(
                        None,
                        &declaration.return_type,
                        &file.source_name,
                        ReferenceUsage::ReturnValue,
                    );
                    for failure in &declaration.failures {
                        self.references.push(PendingReference {
                            refined: None,
                            target: failure.text.clone(),
                            expected: ReferenceKind::Failure,
                            usage: ReferenceUsage::FailsEntry,
                            source: file.source_name.clone(),
                            range: failure.range,
                        });
                    }
                    self.collect_block_calls(&callable_name, &declaration.body, &file.source_name);
                }
                Declaration::Fixture(declaration) => {
                    self.add_authored_node(
                        NodeKind::Fixture,
                        &declaration.name.text,
                        &file.source_name,
                        declaration.name.range,
                    );
                    if let Some(clock) = &declaration.clock {
                        self.collect_expression_calls(
                            &format!("fixture:{}", declaration.name.text),
                            clock,
                            &file.source_name,
                        );
                    }
                    if let Some(configuration) = &declaration.configuration {
                        for item in configuration {
                            self.collect_expression_calls(
                                &format!("fixture:{}", declaration.name.text),
                                &item.value,
                                &file.source_name,
                            );
                        }
                    }
                }
                Declaration::Test(declaration) => {
                    let name = format!("test:{}", declaration.name.text);
                    self.add_authored_node(
                        NodeKind::Test,
                        &name,
                        &file.source_name,
                        declaration.name.range,
                    );
                    if let Some(fixture) = &declaration.fixture {
                        self.references.push(PendingReference {
                            refined: None,
                            target: fixture.text.clone(),
                            expected: ReferenceKind::Fixture,
                            usage: ReferenceUsage::TestFixture,
                            source: file.source_name.clone(),
                            range: fixture.range,
                        });
                    }
                    self.collect_block_calls(&name, &declaration.body, &file.source_name);
                }
                Declaration::Route(declaration) => {
                    let route_name = format!(
                        "{} {}",
                        http_method_name(declaration.method),
                        declaration.path
                    );
                    self.add_authored_node(
                        NodeKind::Route,
                        &route_name,
                        &file.source_name,
                        declaration.range,
                    );
                    if let Some(input) = &declaration.input {
                        self.add_type_reference(
                            None,
                            input,
                            &file.source_name,
                            ReferenceUsage::RouteInput,
                        );
                    }
                    if let Some(output) = &declaration.output {
                        self.add_type_reference(
                            None,
                            output,
                            &file.source_name,
                            ReferenceUsage::RouteOutput,
                        );
                    }
                    if let Some(run) = &declaration.run {
                        self.calls.push(PendingCall {
                            caller: route_name.clone(),
                            callee: name_expression(&run.callee.path),
                            source: file.source_name.clone(),
                            range: run.callee.range,
                        });
                    }
                    if let Some(action) = &declaration.inline_action {
                        self.collect_block_calls(&route_name, &action.body, &file.source_name);
                    }
                }
            }
        }
    }

    fn validate_relationships(&mut self, files: &[ParsedSyntax]) {
        let mut entities = BTreeMap::new();
        for file in files {
            for declaration in &file.file.declarations {
                if let Declaration::Record(record) = declaration {
                    if record.kind == RecordKind::Entity {
                        entities.insert(
                            record.name.text.clone(),
                            (record, file.source_name.as_str()),
                        );
                    }
                }
            }
        }

        let mut dependencies = entities
            .keys()
            .map(|name| (name.clone(), BTreeSet::new()))
            .collect::<BTreeMap<_, _>>();

        for (entity_name, (entity, source)) in &entities {
            for field in &entity.fields {
                let Some(reference) = &field.reference else {
                    continue;
                };
                let target_parts = reference
                    .target
                    .path
                    .iter()
                    .map(|part| part.text.as_str())
                    .collect::<Vec<_>>();
                if target_parts.len() != 2
                    || !reference.target.arguments.is_empty()
                    || reference.target.nullable
                {
                    let diagnostic = Diagnostic::error("DATA_RELATIONSHIP_TARGET_FIELD");
                    self.diagnostics
                        .push(with_span(diagnostic, source, reference.target.range));
                    continue;
                }

                let target_entity_name = target_parts[0];
                let target_field_name = target_parts[1];
                let Some((target_entity, _)) = entities.get(target_entity_name) else {
                    continue;
                };
                let Some(target_field) = target_entity
                    .fields
                    .iter()
                    .find(|candidate| candidate.name.text == target_field_name)
                else {
                    let diagnostic = Diagnostic::error("DATA_RELATIONSHIP_TARGET_FIELD");
                    self.diagnostics
                        .push(with_span(diagnostic, source, reference.target.range));
                    continue;
                };

                if self.reference_is_nullable(&target_field.field_type)
                    || (!target_field
                        .persistence
                        .contains(&PersistenceModifier::Identity)
                        && !target_field
                            .persistence
                            .contains(&PersistenceModifier::Unique))
                {
                    let diagnostic = Diagnostic::error("DATA_RELATIONSHIP_TARGET_NOT_KEY");
                    self.diagnostics
                        .push(with_span(diagnostic, source, reference.target.range));
                }

                let source_type = field
                    .field_type
                    .path
                    .iter()
                    .map(|part| part.text.as_str())
                    .collect::<Vec<_>>();
                if source_type != target_parts || !field.field_type.arguments.is_empty() {
                    let diagnostic = Diagnostic::error("DATA_RELATIONSHIP_TYPE_MISMATCH");
                    self.diagnostics
                        .push(with_span(diagnostic, source, field.field_type.range));
                }

                if reference.on_delete == ReferenceDeleteAction::SetNull
                    && !self.reference_is_nullable(&field.field_type)
                {
                    let diagnostic = Diagnostic::error("DATA_RELATIONSHIP_SET_NULL_REQUIRED");
                    self.diagnostics
                        .push(with_span(diagnostic, source, reference.range));
                }

                if target_entity_name != entity_name {
                    dependencies
                        .entry(entity_name.clone())
                        .or_default()
                        .insert(target_entity_name.to_owned());
                }
            }

            let mut inverse_names = BTreeSet::new();
            for inverse in &entity.inverses {
                if !inverse_names.insert(inverse.name.text.as_str())
                    || entity
                        .fields
                        .iter()
                        .any(|field| field.name.text == inverse.name.text)
                {
                    let diagnostic = Diagnostic::error("DATA_INVERSE_DUPLICATE_NAME");
                    self.diagnostics
                        .push(with_span(diagnostic, source, inverse.name.range));
                }

                let via_parts = inverse
                    .via
                    .path
                    .iter()
                    .map(|part| part.text.as_str())
                    .collect::<Vec<_>>();
                if via_parts.len() != 2
                    || !inverse.via.arguments.is_empty()
                    || inverse.via.nullable
                    || via_parts.first().copied() != Some(inverse.target.text.as_str())
                {
                    let diagnostic = Diagnostic::error("DATA_INVERSE_VIA_FIELD");
                    self.diagnostics
                        .push(with_span(diagnostic, source, inverse.via.range));
                    continue;
                }
                let child_name = via_parts[0];
                let child_field_name = via_parts[1];
                let Some((child, _)) = entities.get(child_name) else {
                    continue;
                };
                let Some(child_field) = child
                    .fields
                    .iter()
                    .find(|field| field.name.text == child_field_name)
                else {
                    let diagnostic = Diagnostic::error("DATA_INVERSE_VIA_FIELD");
                    self.diagnostics
                        .push(with_span(diagnostic, source, inverse.via.range));
                    continue;
                };
                let points_back = child_field.reference.as_ref().is_some_and(|reference| {
                    reference.target.path.len() == 2
                        && reference.target.path[0].text == *entity_name
                });
                if !points_back {
                    let diagnostic = Diagnostic::error("DATA_INVERSE_NOT_OWNING_REFERENCE");
                    self.diagnostics
                        .push(with_span(diagnostic, source, inverse.via.range));
                }
                if inverse.cardinality == jadpo_syntax::InverseCardinality::Optional
                    && !child_field.persistence.iter().any(|modifier| {
                        matches!(
                            modifier,
                            PersistenceModifier::Identity | PersistenceModifier::Unique
                        )
                    })
                {
                    let diagnostic = Diagnostic::error("DATA_INVERSE_OPTIONAL_NOT_UNIQUE");
                    self.diagnostics
                        .push(with_span(diagnostic, source, inverse.via.range));
                }
            }
        }

        let mut emitted = BTreeSet::new();
        loop {
            let ready = dependencies
                .iter()
                .filter(|(name, required)| {
                    !emitted.contains(*name)
                        && required
                            .iter()
                            .all(|dependency| emitted.contains(dependency))
                })
                .map(|(name, _)| name.clone())
                .collect::<Vec<_>>();
            if ready.is_empty() {
                break;
            }
            emitted.extend(ready);
        }

        if emitted.len() != dependencies.len() {
            for (entity_name, (entity, source)) in &entities {
                if emitted.contains(entity_name) {
                    continue;
                }
                if let Some(reference) = entity.fields.iter().find_map(|field| {
                    field.reference.as_ref().filter(|reference| {
                        reference
                            .target
                            .path
                            .first()
                            .is_some_and(|target| !emitted.contains(&target.text))
                    })
                }) {
                    let diagnostic = Diagnostic::error("DATA_RELATIONSHIP_CYCLE");
                    self.diagnostics
                        .push(with_span(diagnostic, source, reference.range));
                }
            }
        }
    }

    fn add_node(&mut self, node: PendingNode) {
        if let Some(previous) = self.nodes.get(&node.name) {
            let diagnostic = Diagnostic::error("SEM_DUPLICATE_DECLARATION")
                .with_fact(DiagnosticFact::Name(node.name.clone()))
                .with_note(format!(
                    "first declaration is at {}:{}..{}",
                    previous.source, previous.range.start, previous.range.end
                ));
            self.diagnostics
                .push(with_span(diagnostic, &node.source, node.range));
            return;
        }
        self.nodes.insert(node.name.clone(), node);
    }

    fn add_authored_node(&mut self, kind: NodeKind, name: &str, source: &str, range: TextRange) {
        if is_standard_namespace(name) {
            return;
        }
        self.add_node(PendingNode {
            kind,
            name: name.to_owned(),
            source: source.to_owned(),
            range,
        });
    }

    fn add_type_reference(
        &mut self,
        refined: Option<String>,
        reference: &TypeReference,
        source: &str,
        usage: ReferenceUsage,
    ) {
        if reference.nullable {
            if let Some(name) = &refined {
                self.nullable_types.insert(name.clone());
            }
            self.nullable_references.push((
                name_expression(&reference.path),
                source.to_owned(),
                reference.range,
            ));
        }
        self.references.push(PendingReference {
            refined,
            target: name_expression(&reference.path),
            expected: ReferenceKind::Type,
            usage,
            source: source.to_owned(),
            range: reference.range,
        });
        for argument in &reference.arguments {
            self.add_type_reference(None, argument, source, usage);
        }
    }

    fn collect_block_calls(&mut self, caller: &str, block: &Block, source: &str) {
        for statement in &block.statements {
            match statement {
                Statement::Binding(statement) => {
                    if let Some(annotation) = &statement.annotation {
                        self.add_type_reference(
                            None,
                            annotation,
                            source,
                            ReferenceUsage::LocalAnnotation,
                        );
                    }
                    self.collect_expression_calls(caller, &statement.value, source)
                }
                Statement::Assignment(statement) => {
                    self.collect_expression_calls(caller, &statement.value, source)
                }
                Statement::Return(statement) => {
                    self.collect_expression_calls(caller, &statement.value, source)
                }
                Statement::Reject(statement) => {
                    for field in &statement.values {
                        self.collect_expression_calls(caller, &field.value, source);
                    }
                }
                Statement::If(statement) => {
                    self.collect_expression_calls(caller, &statement.condition, source);
                    self.collect_block_calls(caller, &statement.then_block, source);
                    if let Some(else_block) = &statement.else_block {
                        self.collect_block_calls(caller, else_block, source);
                    }
                }
                Statement::Match(statement) => {
                    self.collect_expression_calls(caller, &statement.subject, source);
                    for arm in &statement.arms {
                        self.collect_block_calls(caller, &arm.body, source);
                    }
                }
                Statement::Assert(statement) => {
                    self.collect_expression_calls(caller, &statement.condition, source)
                }
                Statement::AdvanceClock(statement) => {
                    self.collect_expression_calls(caller, &statement.duration, source)
                }
                Statement::Unsupported(_) => {}
            }
        }
    }

    fn collect_expression_calls(&mut self, caller: &str, expression: &Expression, source: &str) {
        match expression {
            Expression::Invocation(invocation) => {
                self.calls.push(PendingCall {
                    caller: caller.to_owned(),
                    callee: name_expression(&invocation.callee.path),
                    source: source.to_owned(),
                    range: invocation.callee.range,
                });
                for argument in &invocation.arguments {
                    self.collect_expression_calls(caller, argument, source);
                }
                for argument in &invocation.named_arguments {
                    self.collect_expression_calls(caller, &argument.value, source);
                }
            }
            Expression::TestCall(call) => {
                self.calls.push(PendingCall {
                    caller: caller.to_owned(),
                    callee: name_expression(&call.invocation.callee.path),
                    source: source.to_owned(),
                    range: call.invocation.callee.range,
                });
                for argument in &call.invocation.arguments {
                    self.collect_expression_calls(caller, argument, source);
                }
                for argument in &call.invocation.named_arguments {
                    self.collect_expression_calls(caller, &argument.value, source);
                }
            }
            Expression::Construction(construction) => {
                for field in &construction.fields {
                    self.collect_expression_calls(caller, &field.value, source);
                }
            }
            Expression::Object(object) => {
                for field in &object.fields {
                    self.collect_expression_calls(caller, &field.value, source);
                }
            }
            Expression::Create(create) => {
                for field in &create.fields {
                    self.collect_expression_calls(caller, &field.value, source);
                }
                for conflict in &create.conflicts {
                    for field in &conflict.rejection.values {
                        self.collect_expression_calls(caller, &field.value, source);
                    }
                }
            }
            Expression::Query(query) => {
                self.collect_expression_calls(caller, &query.value, source);
                if let Some(pagination) = &query.pagination {
                    self.collect_expression_calls(caller, &pagination.limit, source);
                    self.collect_expression_calls(caller, &pagination.offset, source);
                }
                for include in &query.includes {
                    self.collect_expression_calls(caller, &include.pagination.limit, source);
                    self.collect_expression_calls(caller, &include.pagination.offset, source);
                }
                if let Some(missing) = &query.missing {
                    for field in &missing.values {
                        self.collect_expression_calls(caller, &field.value, source);
                    }
                }
            }
            Expression::Update(update) => {
                self.collect_expression_calls(caller, &update.value, source);
                for change in &update.changes {
                    self.collect_expression_calls(caller, &change.value, source);
                }
                for conditional in &update.conditional_changes {
                    self.collect_expression_calls(caller, &conditional.change.value, source);
                }
                for binding in update
                    .empty
                    .iter()
                    .chain(std::iter::once(&update.missing))
                    .chain(update.conflicts.iter().map(|conflict| &conflict.rejection))
                {
                    for field in &binding.values {
                        self.collect_expression_calls(caller, &field.value, source);
                    }
                }
            }
            Expression::Delete(delete) => {
                self.collect_expression_calls(caller, &delete.value, source);
                for binding in std::iter::once(&delete.missing)
                    .chain(delete.conflicts.iter().map(|conflict| &conflict.rejection))
                {
                    for field in &binding.values {
                        self.collect_expression_calls(caller, &field.value, source);
                    }
                }
            }
            Expression::Binary(binary) => {
                self.collect_expression_calls(caller, &binary.left, source);
                self.collect_expression_calls(caller, &binary.right, source);
            }
            Expression::Unary(unary) => self.collect_expression_calls(caller, &unary.value, source),
            Expression::Grouped(grouped) => {
                self.collect_expression_calls(caller, &grouped.value, source)
            }
            Expression::Attempt(attempt) => {
                self.collect_expression_calls(caller, &attempt.value, source)
            }
            Expression::OutcomeMatch(outcome) => {
                self.collect_expression_calls(caller, &outcome.subject, source);
                for arm in &outcome.arms {
                    match &arm.body {
                        jadpo_syntax::OutcomeMatchArmBody::Value(value) => {
                            self.collect_expression_calls(caller, value, source)
                        }
                        jadpo_syntax::OutcomeMatchArmBody::Reject(rejection) => {
                            for field in &rejection.values {
                                self.collect_expression_calls(caller, &field.value, source);
                            }
                        }
                        jadpo_syntax::OutcomeMatchArmBody::Propagate(_) => {}
                    }
                }
            }
            Expression::Literal(_) | Expression::Name(_) | Expression::Missing(_) => {}
        }
    }

    fn reference_is_nullable(&self, reference: &TypeReference) -> bool {
        reference.nullable
            || self
                .nullable_types
                .contains(&name_expression(&reference.path))
    }

    fn resolve_nullability(&mut self) {
        // Resolve nullability to a fixed point so declaration order and chains of
        // field refinements do not erase absence. The finite named graph bounds
        // this walk even when another diagnostic rejects a refinement cycle.
        loop {
            let before = self.nullable_types.len();
            for reference in &self.references {
                if self.nullable_types.contains(&reference.target) {
                    if let Some(refined) = &reference.refined {
                        self.nullable_types.insert(refined.clone());
                    }
                }
            }
            if self.nullable_types.len() == before {
                break;
            }
        }
        for (target, source, range) in &self.nullable_references {
            if self.nullable_types.contains(target) {
                self.diagnostics.push(with_span(
                    Diagnostic::error("TYPE_REDUNDANT_NULLABILITY")
                        .with_fact(DiagnosticFact::Name(target.clone())),
                    source,
                    *range,
                ));
            }
        }
    }

    fn validate_storage_nullability(&mut self, files: &[ParsedSyntax]) {
        for file in files {
            for declaration in &file.file.declarations {
                let Declaration::Record(record) = declaration else {
                    continue;
                };
                for field in &record.fields {
                    if field.persistence.contains(&PersistenceModifier::Identity)
                        && self.reference_is_nullable(&field.field_type)
                    {
                        self.diagnostics.push(with_span(
                            Diagnostic::error("DATA_IDENTITY_NULLABLE"),
                            &file.source_name,
                            field.range,
                        ));
                    }
                }
                for constraint in &record.persistence_constraints {
                    let mut seen = BTreeSet::new();
                    for name in &constraint.fields {
                        if !seen.insert(&name.text) {
                            continue;
                        }
                        let Some(field) = record
                            .fields
                            .iter()
                            .find(|field| field.name.text == name.text)
                        else {
                            continue;
                        };
                        if self.reference_is_nullable(&field.field_type) {
                            self.diagnostics.push(with_span(
                                Diagnostic::error("DATA_CONSTRAINT_NULLABLE_FIELD"),
                                &file.source_name,
                                name.range,
                            ));
                        }
                    }
                }
            }
        }
    }

    fn finish(mut self) -> SemanticGraph {
        if let Some(principal) = &self.principal {
            for (variant, source, range) in &self.authentication_principals {
                self.references.push(PendingReference {
                    refined: None,
                    target: format!("{principal}.{variant}"),
                    expected: ReferenceKind::PrincipalVariant,
                    usage: ReferenceUsage::AuthenticationPrincipal,
                    source: source.clone(),
                    range: *range,
                });
            }
        }
        let nodes = self
            .nodes
            .values()
            .enumerate()
            .map(|(index, node)| SemanticNode {
                id: NodeId(index as u32),
                kind: node.kind,
                name: node.name.clone(),
                source: node.source.clone(),
                range: node.range,
            })
            .collect::<Vec<_>>();
        let ids = nodes
            .iter()
            .map(|node| (node.name.clone(), node.id))
            .collect::<BTreeMap<_, _>>();
        let kinds = nodes
            .iter()
            .map(|node| (node.name.clone(), node.kind))
            .collect::<BTreeMap<_, _>>();
        let mut refinements = Vec::new();

        for reference in self.references {
            let Some(target) = ids.get(&reference.target).copied() else {
                let mut diagnostic = Diagnostic::error("SEM_UNKNOWN_NAME")
                    .with_fact(DiagnosticFact::Name(reference.target.clone()))
                    .with_fact(DiagnosticFact::Expected(
                        reference.expected.user_name().to_owned(),
                    ))
                    .with_fact(DiagnosticFact::Usage(
                        reference.usage.user_name().to_owned(),
                    ));
                if let Some(examples) = reference.expected.examples() {
                    diagnostic =
                        diagnostic.with_fact(DiagnosticFact::Examples(examples.to_owned()));
                }
                if let Some(suggestion) = closest_semantic_name(
                    &reference.target,
                    nodes
                        .iter()
                        .filter(|node| reference.expected.accepts(node.kind))
                        .map(|node| node.name.as_str()),
                ) {
                    diagnostic = diagnostic.with_fact(DiagnosticFact::SuggestedName(suggestion));
                }
                self.diagnostics
                    .push(with_span(diagnostic, &reference.source, reference.range));
                continue;
            };
            if let Some(diagnostic) = module_visibility_diagnostic(
                &self.module_scopes,
                &nodes[target.0 as usize],
                &reference.target,
                &reference.source,
                reference.range,
            ) {
                self.diagnostics.push(diagnostic);
                continue;
            }
            let target_kind = kinds[&reference.target];
            let kind_matches = match reference.expected {
                ReferenceKind::Type => target_kind.is_type(),
                ReferenceKind::Principal => matches!(target_kind, NodeKind::Principal),
                ReferenceKind::PrincipalVariant => {
                    matches!(target_kind, NodeKind::PrincipalVariant)
                }
                ReferenceKind::AuthenticationField => matches!(target_kind, NodeKind::Field),
                ReferenceKind::Failure => {
                    matches!(target_kind, NodeKind::Failure | NodeKind::StandardFailure)
                }
                ReferenceKind::StandardFailure => target_kind == NodeKind::StandardFailure,
                ReferenceKind::Fixture => target_kind == NodeKind::Fixture,
            };
            if !kind_matches {
                let mut diagnostic = Diagnostic::error("SEM_WRONG_NAME_KIND")
                    .with_fact(DiagnosticFact::Name(reference.target.clone()))
                    .with_fact(DiagnosticFact::Expected(
                        reference.expected.user_name().to_owned(),
                    ))
                    .with_fact(DiagnosticFact::ActualKind(
                        target_kind.user_name().to_owned(),
                    ))
                    .with_fact(DiagnosticFact::Usage(
                        reference.usage.user_name().to_owned(),
                    ));
                if let Some(examples) = reference.expected.examples() {
                    diagnostic =
                        diagnostic.with_fact(DiagnosticFact::Examples(examples.to_owned()));
                }
                self.diagnostics
                    .push(with_span(diagnostic, &reference.source, reference.range));
                continue;
            }

            if let Some(refined_name) = reference.refined {
                if let Some(refined) = ids.get(&refined_name).copied() {
                    refinements.push(RefinementEdge {
                        refined,
                        parent: target,
                    });
                }
            }
        }

        let mut calls = Vec::new();
        for call in self.calls {
            let Some(caller) = ids.get(&call.caller).copied() else {
                continue;
            };
            let resolved_callee = if ids.contains_key(&call.callee) {
                call.callee.clone()
            } else {
                let operation = call.callee.rsplit('.').next().unwrap_or(&call.callee);
                let mut candidates = nodes
                    .iter()
                    .filter(|node| {
                        matches!(
                            node.kind,
                            NodeKind::Function | NodeKind::Action | NodeKind::Query
                        ) && node.name.ends_with(&format!(".{operation}"))
                    })
                    .map(|node| node.name.clone());
                match (candidates.next(), candidates.next()) {
                    (Some(candidate), None) => candidate,
                    _ => call.callee.clone(),
                }
            };
            let Some(callee) = ids.get(&resolved_callee).copied() else {
                if let Some(namespace) = standard_operation_namespace(
                    call.callee.rsplit('.').next().unwrap_or(&call.callee),
                ) {
                    let operation = call.callee.rsplit('.').next().unwrap_or(&call.callee);
                    let diagnostic = Diagnostic::error("SEM_STANDARD_OPERATION_QUALIFICATION")
                        .with_fact(DiagnosticFact::Name(call.callee.clone()))
                        .with_fact(DiagnosticFact::SuggestedName(format!(
                            "{namespace}.{operation}"
                        )));
                    self.diagnostics
                        .push(with_span(diagnostic, &call.source, call.range));
                    continue;
                }
                let mut diagnostic = Diagnostic::error("SEM_UNKNOWN_CALLEE")
                    .with_fact(DiagnosticFact::Name(call.callee.clone()))
                    .with_fact(DiagnosticFact::Expected("function or action".to_owned()))
                    .with_fact(DiagnosticFact::Usage("call".to_owned()));
                if let Some(suggestion) = closest_semantic_name(
                    &call.callee,
                    nodes
                        .iter()
                        .filter(|node| {
                            matches!(
                                node.kind,
                                NodeKind::Function
                                    | NodeKind::Action
                                    | NodeKind::Query
                                    | NodeKind::StandardFunction
                            )
                        })
                        .map(|node| node.name.as_str()),
                ) {
                    diagnostic = diagnostic.with_fact(DiagnosticFact::SuggestedName(suggestion));
                }
                self.diagnostics
                    .push(with_span(diagnostic, &call.source, call.range));
                continue;
            };
            if let Some(diagnostic) = module_visibility_diagnostic(
                &self.module_scopes,
                &nodes[callee.0 as usize],
                &resolved_callee,
                &call.source,
                call.range,
            ) {
                self.diagnostics.push(diagnostic);
                continue;
            }
            match kinds[&resolved_callee] {
                NodeKind::Function
                | NodeKind::Action
                | NodeKind::Query
                | NodeKind::StandardFunction => calls.push(CallEdge { caller, callee }),
                kind if kind.is_type() => {}
                actual => {
                    let diagnostic = Diagnostic::error("SEM_NOT_CALLABLE")
                        .with_fact(DiagnosticFact::Name(call.callee.clone()))
                        .with_fact(DiagnosticFact::Expected("function or action".to_owned()))
                        .with_fact(DiagnosticFact::ActualKind(actual.user_name().to_owned()));
                    self.diagnostics
                        .push(with_span(diagnostic, &call.source, call.range));
                }
            }
        }

        refinements.sort_by_key(|edge| (edge.refined, edge.parent));
        refinements.dedup();
        calls.sort_by_key(|edge| (edge.caller, edge.callee));
        calls.dedup();
        let mut authentication_resolutions = self
            .authentication_resolutions
            .into_iter()
            .filter_map(|edge| {
                Some(AuthenticationResolutionEdge {
                    resolution: *ids.get(&edge.resolution)?,
                    authority: *ids.get(&edge.authority)?,
                })
            })
            .collect::<Vec<_>>();
        authentication_resolutions.sort_by_key(|edge| (edge.resolution, edge.authority));
        authentication_resolutions.dedup();

        SemanticGraph {
            modules: self.modules,
            nodes,
            refinements,
            nullable_types: self.nullable_types,
            calls,
            authentication_resolutions,
            diagnostics: self.diagnostics,
        }
    }
}

fn module_visibility_diagnostic(
    scopes: &BTreeMap<String, ModuleScope>,
    target_node: &SemanticNode,
    target_name: &str,
    source: &str,
    range: TextRange,
) -> Option<Diagnostic> {
    let scope = scopes.get(source)?;
    if target_node.source == source || target_node.source == "<prelude>" {
        return None;
    }
    let root_name = target_name.split('.').next().unwrap_or(target_name);
    if scope.imported_names.contains(root_name) {
        return None;
    }
    Some(with_span(
        Diagnostic::error("MOD_IMPORT_REQUIRED"),
        source,
        range,
    ))
}

#[derive(Clone, Copy)]
enum NameCase {
    UpperCamel,
    LowerSnake,
}

impl NameCase {
    fn accepts(self, value: &str) -> bool {
        match self {
            Self::UpperCamel => {
                value
                    .bytes()
                    .next()
                    .is_some_and(|byte| byte.is_ascii_uppercase())
                    && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
            }
            Self::LowerSnake => {
                value
                    .bytes()
                    .next()
                    .is_some_and(|byte| byte.is_ascii_lowercase())
                    && value.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
                    })
                    && !value.ends_with('_')
                    && !value.contains("__")
            }
        }
    }

    const fn description(self) -> &'static str {
        match self {
            Self::UpperCamel => "UpperCamelCase",
            Self::LowerSnake => "lower_snake_case",
        }
    }

    fn convert(self, value: &str) -> String {
        let snake = to_lower_snake(value);
        match self {
            Self::LowerSnake => snake,
            Self::UpperCamel => snake
                .split('_')
                .filter(|part| !part.is_empty())
                .map(|part| {
                    let mut characters = part.chars();
                    characters.next().map_or_else(String::new, |first| {
                        first.to_uppercase().chain(characters).collect()
                    })
                })
                .collect(),
        }
    }
}

fn to_lower_snake(value: &str) -> String {
    let mut output = String::new();
    let mut previous_was_separator = true;
    for character in value.chars() {
        if character == '_' || character == '-' {
            if !previous_was_separator && !output.is_empty() {
                output.push('_');
            }
            previous_was_separator = true;
            continue;
        }
        if character.is_ascii_uppercase() {
            if !previous_was_separator && !output.is_empty() {
                output.push('_');
            }
            output.push(character.to_ascii_lowercase());
        } else if character.is_ascii_alphanumeric() {
            output.push(character.to_ascii_lowercase());
        }
        previous_was_separator = false;
    }
    while output.ends_with('_') {
        output.pop();
    }
    output
}

fn is_standard_namespace(value: &str) -> bool {
    matches!(value, "temporal" | "collection")
}

fn standard_operation_namespace(value: &str) -> Option<&'static str> {
    matches!(
        value,
        "in_zone"
            | "resolve"
            | "add_elapsed"
            | "between"
            | "add_days"
            | "add_weeks"
            | "add_months"
            | "add_years"
            | "add_local_days"
            | "add_local_weeks"
            | "add_local_months"
            | "add_local_years"
            | "day_bounds"
            | "week_bounds"
            | "month_bounds"
            | "year_bounds"
            | "calendar_date"
            | "year"
            | "month"
            | "day"
            | "weekday"
            | "hour"
            | "minute"
            | "second"
            | "millisecond"
            | "offset"
            | "zone"
            | "same_zone"
            | "same_local"
            | "format"
            | "format_friendly"
    )
    .then_some("temporal")
}

fn declaration_name(declaration: &Declaration) -> Option<&jadpo_syntax::Name> {
    match declaration {
        Declaration::Application(declaration) => Some(&declaration.name),
        Declaration::Locales(_) => None,
        Declaration::AuthenticationStrategy(declaration) => Some(&declaration.name),
        Declaration::Principal(declaration) => Some(&declaration.name),
        Declaration::Config(declaration) => Some(&declaration.name),
        Declaration::Type(declaration) => Some(&declaration.name),
        Declaration::Enum(declaration) => Some(&declaration.name),
        Declaration::Record(declaration) => Some(&declaration.name),
        Declaration::Failure(declaration) => Some(&declaration.name),
        Declaration::Callable(declaration) => Some(&declaration.name),
        Declaration::Fixture(declaration) => Some(&declaration.name),
        Declaration::Test(_) => None,
        Declaration::Route(_) => None,
    }
}

fn positive_duration(value: &str) -> bool {
    value
        .trim_end_matches(|character: char| character.is_ascii_alphabetic())
        .parse::<f64>()
        .is_ok_and(|value| value.is_finite() && value > 0.0)
}

fn unquote_string(value: &str) -> String {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
        .to_owned()
}

fn valid_configuration_binding(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn name_expression(path: &[jadpo_syntax::Name]) -> String {
    path.iter()
        .map(|name| name.text.as_str())
        .collect::<Vec<_>>()
        .join(".")
}

fn standard_variant_name(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut separator = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if separator && !result.is_empty() {
                result.push('_');
            }
            result.push(character.to_ascii_lowercase());
            separator = false;
        } else {
            separator = true;
        }
    }
    result
}

fn closest_semantic_name<'a>(
    requested: &str,
    candidates: impl Iterator<Item = &'a str>,
) -> Option<String> {
    let requested_lower = requested.to_ascii_lowercase();
    candidates
        .filter(|candidate| *candidate != requested)
        .filter_map(|candidate| {
            let distance = edit_distance(&requested_lower, &candidate.to_ascii_lowercase());
            (distance <= 3).then_some((distance, candidate))
        })
        .min_by(|left, right| left.cmp(right))
        .map(|(_, candidate)| candidate.to_owned())
}

fn edit_distance(left: &str, right: &str) -> usize {
    let mut previous = (0..=right.chars().count()).collect::<Vec<_>>();
    for (left_index, left_character) in left.chars().enumerate() {
        let mut current = vec![left_index + 1];
        for (right_index, right_character) in right.chars().enumerate() {
            current.push(
                (previous[right_index + 1] + 1).min(
                    (current[right_index] + 1).min(
                        previous[right_index] + usize::from(left_character != right_character),
                    ),
                ),
            );
        }
        previous = current;
    }
    previous.last().copied().unwrap_or_default()
}

fn http_method_name(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Delete => "DELETE",
    }
}

fn with_span(mut diagnostic: Diagnostic, source: &str, range: TextRange) -> Diagnostic {
    diagnostic.primary = Some(SourceSpan {
        source: source.to_owned(),
        start: range.start,
        end: range.end,
    });
    diagnostic
}

fn escape_json(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| match character {
            '\"' => "\\\"".chars().collect::<Vec<_>>(),
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '\n' => "\\n".chars().collect::<Vec<_>>(),
            '\r' => "\\r".chars().collect::<Vec<_>>(),
            '\t' => "\\t".chars().collect::<Vec<_>>(),
            other => vec![other],
        })
        .collect()
}

fn json_strings(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| format!("\"{}\"", escape_json(value)))
            .collect::<Vec<_>>()
            .join(",")
    )
}

#[cfg(test)]
mod tests {
    use super::{build_semantic_graph, NodeKind, ScaffoldManifest};
    use jadpo_syntax::parse;
    use std::path::Path;

    #[test]
    fn manifest_json_escapes_source_paths() {
        let manifest = ScaffoldManifest {
            schema_version: 1,
            phase: "scaffold",
            source_files: vec!["a/quoted\"file.jadpo".to_owned()],
        };
        assert_eq!(
            manifest.to_json(),
            "{\"schema_version\":1,\"phase\":\"scaffold\",\"source_files\":[\"a/quoted\\\"file.jadpo\"]}"
        );
    }

    #[test]
    fn builds_deterministic_field_and_refinement_nodes() {
        let source = r#"
entity Customer { email: Email }
"#;
        let parsed = parse(Path::new("app.jadpo"), source);
        let graph = build_semantic_graph(&[parsed]);

        assert!(graph.diagnostics.is_empty(), "{:#?}", graph.diagnostics);
        assert_eq!(graph.node("Email").unwrap().kind, NodeKind::Type);
        assert_eq!(graph.node("Customer.email").unwrap().kind, NodeKind::Field);
        let edges = graph
            .refinements
            .iter()
            .map(|edge| {
                (
                    graph.nodes[edge.refined.0 as usize].name.as_str(),
                    graph.nodes[edge.parent.0 as usize].name.as_str(),
                )
            })
            .collect::<Vec<_>>();
        assert!(edges.contains(&("Email", "Text")));
        assert!(edges.contains(&("Customer.email", "Email")));
    }

    #[test]
    fn enforces_selective_module_imports_visibility_and_cycles() {
        let shared = parse(
            Path::new("shared.jadpo"),
            "module todo.shared\npublic type ContactEmail = Email {}\n",
        );
        let api = parse(
            Path::new("api.jadpo"),
            "module todo.api\nimport todo.shared { ContactEmail }\npublic input Signup { email: ContactEmail }\n",
        );
        assert!(shared.diagnostics.is_empty(), "{:#?}", shared.diagnostics);
        assert!(api.diagnostics.is_empty(), "{:#?}", api.diagnostics);
        let graph = build_semantic_graph(&[shared.clone(), api]);
        assert!(graph.diagnostics.is_empty(), "{:#?}", graph.diagnostics);
        assert_eq!(graph.modules.len(), 2);
        assert_eq!(graph.modules[0].name, "todo.api");
        assert_eq!(graph.modules[0].imports, vec!["todo.shared.ContactEmail"]);
        assert!(graph
            .manifest()
            .to_json()
            .contains("\"modules\":[{\"name\":\"todo.api\""));

        let missing_import = parse(
            Path::new("missing.jadpo"),
            "module todo.consumer\ninput SignupWithoutImport { email: ContactEmail }\n",
        );
        let graph = build_semantic_graph(&[shared, missing_import]);
        assert!(
            graph
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "MOD_IMPORT_REQUIRED"),
            "{:#?}",
            graph.diagnostics
        );

        let private_source = parse(
            Path::new("private.jadpo"),
            "module private.jadpo\ntype Hidden = Text {}\n",
        );
        let private_user = parse(
            Path::new("private-user.jadpo"),
            "module private.user\nimport private.jadpo { Hidden }\ninput UsesHidden { value: Hidden }\n",
        );
        let graph = build_semantic_graph(&[private_source, private_user]);
        assert!(graph
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "MOD_PRIVATE_IMPORT"));

        let cycle_a = parse(
            Path::new("cycle-a.jadpo"),
            "module cycle.a\nimport cycle.b { B }\npublic type A = B {}\n",
        );
        let cycle_b = parse(
            Path::new("cycle-b.jadpo"),
            "module cycle.b\nimport cycle.a { A }\npublic type B = A {}\n",
        );
        let graph = build_semantic_graph(&[cycle_a, cycle_b]);
        assert!(graph
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "MOD_IMPORT_CYCLE"));
    }

    #[test]
    fn reports_each_invalid_module_relationship() {
        let parse_file = |name: &str, source: &str| {
            let parsed = parse(Path::new(name), source);
            assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
            parsed
        };
        let codes = |files: Vec<_>| {
            build_semantic_graph(&files)
                .diagnostics
                .into_iter()
                .map(|diagnostic| diagnostic.code)
                .collect::<Vec<_>>()
        };

        let without_module = codes(vec![parse_file(
            "public-without-module.jadpo",
            "public type PublicName = Text {}\ntype LocalName = Text {}\n",
        )]);
        assert!(
            without_module.contains(&"MOD_PUBLIC_REQUIRES_MODULE"),
            "{without_module:#?}"
        );

        let duplicate_modules = codes(vec![
            parse_file(
                "first.jadpo",
                "module duplicate.name\ntype First = Text {}\n",
            ),
            parse_file(
                "second.jadpo",
                "module duplicate.name\ntype Second = Text {}\n",
            ),
        ]);
        assert!(
            duplicate_modules.contains(&"MOD_DUPLICATE_MODULE"),
            "{duplicate_modules:#?}"
        );

        let mixed_modules = codes(vec![
            parse_file(
                "explicit.jadpo",
                "module explicit.name\ntype Named = Text {}\n",
            ),
            parse_file("implicit.jadpo", "type Unnamed = Text {}\n"),
        ]);
        assert!(
            mixed_modules.contains(&"MOD_MODULE_REQUIRED"),
            "{mixed_modules:#?}"
        );

        let invalid_imports = codes(vec![
            parse_file(
                "shared.jadpo",
                "module shared.names\npublic type Exported = Text {}\npublic type Extra = Text {}\ntype Private = Text {}\n",
            ),
            parse_file(
                "consumer.jadpo",
                r#"module consumer.names
import consumer.names { Local }
import absent.package { Missing }
import shared.names { Exported, Extra, Extra, Unknown }
type Exported = Text {}
type Local = Text {}
"#,
            ),
        ]);
        for expected in [
            "MOD_SELF_IMPORT",
            "MOD_UNKNOWN_MODULE",
            "MOD_IMPORT_CONFLICT",
            "MOD_DUPLICATE_IMPORT",
            "MOD_UNKNOWN_EXPORT",
        ] {
            assert!(
                invalid_imports.contains(&expected),
                "{expected}: {invalid_imports:#?}"
            );
        }
    }

    #[test]
    fn reports_duplicate_and_unknown_semantic_names() {
        let source = r#"
entity Customer { id: Missing }
entity Customer { id: Uuid }
"#;
        let parsed = parse(Path::new("bad.jadpo"), source);
        let graph = build_semantic_graph(&[parsed]);
        let codes = graph
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>();

        assert!(codes.contains(&"SEM_DUPLICATE_DECLARATION"));
        assert!(codes.contains(&"SEM_UNKNOWN_NAME"));
    }

    #[test]
    fn validates_entity_persistence_modifiers() {
        let source = r#"
value Token { value: Text unique }
entity Account {
    first_id: Uuid identity
    second_id: Uuid? identity
}
"#;
        let parsed = parse(Path::new("bad-persistence.jadpo"), source);
        let graph = build_semantic_graph(&[parsed]);
        let codes = graph
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>();

        assert!(codes.contains(&"DATA_MODIFIER_NON_ENTITY"));
        assert!(codes.contains(&"DATA_IDENTITY_NULLABLE"));
        assert!(codes.contains(&"DATA_MULTIPLE_IDENTITIES"));
    }

    #[test]
    fn validates_compound_persistence_constraints() {
        let source = r#"
entity Account {
    id: Uuid identity
    tenant: Text
    email: Text?
    constraint too_short: unique(id)
    constraint invalid: unique(email, email, absent)
    constraint first_shape: unique(id, tenant)
    constraint repeated_shape: unique(tenant, id)
}
"#;
        let parsed = parse(Path::new("bad-compound-constraints.jadpo"), source);
        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let graph = build_semantic_graph(&[parsed]);
        let codes = graph
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>();

        assert!(codes.contains(&"DATA_COMPOUND_CONSTRAINT_FIELDS"));
        assert!(codes.contains(&"DATA_CONSTRAINT_NULLABLE_FIELD"));
        assert!(codes.contains(&"DATA_CONSTRAINT_DUPLICATE_FIELD"));
        assert!(codes.contains(&"DATA_CONSTRAINT_UNKNOWN_FIELD"));
        assert!(codes.contains(&"DATA_DUPLICATE_CONSTRAINT_SHAPE"));
    }

    #[test]
    fn validates_relationship_targets_lifecycle_and_cycles() {
        let source = r#"
entity User {
    id: Uuid identity
    email: Text
    todo_id: Todo.id references Todo.id on_delete restrict
}
entity Todo {
    id: Uuid identity
    user_id: Text references User.email on_delete set_null
}
"#;
        let parsed = parse(Path::new("bad-relationships.jadpo"), source);
        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let graph = build_semantic_graph(&[parsed]);
        let codes = graph
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>();

        assert!(codes.contains(&"DATA_RELATIONSHIP_TARGET_NOT_KEY"));
        assert!(codes.contains(&"DATA_RELATIONSHIP_TYPE_MISMATCH"));
        assert!(codes.contains(&"DATA_RELATIONSHIP_SET_NULL_REQUIRED"));
        assert!(codes.contains(&"DATA_RELATIONSHIP_CYCLE"));
    }

    #[test]
    fn validates_inverse_relationships_against_owning_references() {
        let source = r#"
entity User {
    id: Uuid identity
    inverse todos: many Todo via Todo.unrelated
}
entity Todo {
    id: Uuid identity
    owner_id: User.id references User.id on_delete cascade
    unrelated: Uuid
}
"#;
        let parsed = parse(Path::new("bad-inverse.jadpo"), source);
        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let graph = build_semantic_graph(&[parsed]);
        assert!(graph
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "DATA_INVERSE_NOT_OWNING_REFERENCE"));
    }

    #[test]
    fn reports_each_invalid_relationship_name_and_target_shape() {
        let source = r#"
entity User {
    id: Uuid identity
    todos: Text
    inverse todos: many Todo via Todo.absent_field
    inverse malformed: many Todo via User.id
}
entity Todo {
    id: Uuid identity
    owner_id: User.id references User on_delete cascade
}
"#;
        let parsed = parse(Path::new("invalid-relationship-shapes.jadpo"), source);
        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let graph = build_semantic_graph(&[parsed]);
        let codes = graph
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>();

        assert!(codes.contains(&"DATA_INVERSE_DUPLICATE_NAME"), "{codes:#?}");
        assert!(codes.contains(&"DATA_INVERSE_VIA_FIELD"), "{codes:#?}");
        assert!(
            codes.contains(&"DATA_RELATIONSHIP_TARGET_FIELD"),
            "{codes:#?}"
        );
    }

    #[test]
    fn indexes_an_explicit_owning_relationship_name() {
        let source = r#"
entity User { id: Uuid identity }
entity Todo {
    id: Uuid identity
    owner_id: User.id references User.id as owner on_delete cascade
}
"#;
        let parsed = parse(Path::new("named-relationship.jadpo"), source);
        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let graph = build_semantic_graph(&[parsed]);

        assert!(graph.diagnostics.is_empty(), "{:#?}", graph.diagnostics);
        assert_eq!(
            graph.node("Todo.owner").unwrap().kind,
            NodeKind::Relationship
        );
        assert_eq!(graph.node("Todo.owner_id").unwrap().kind, NodeKind::Field);
    }

    #[test]
    fn requires_optional_inverse_references_to_be_unique() {
        let source = r#"
entity User {
    id: Uuid identity
    inverse profile: optional Profile via Profile.user_id
}
entity Profile {
    id: Uuid identity
    user_id: User.id references User.id on_delete cascade
}
"#;
        let parsed = parse(Path::new("non-unique-optional-inverse.jadpo"), source);
        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let graph = build_semantic_graph(&[parsed]);
        assert!(graph
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "DATA_INVERSE_OPTIONAL_NOT_UNIQUE"));
    }

    #[test]
    fn validates_configuration_bindings_and_secret_defaults() {
        let source = r#"
config First {
    first: Text { binding: "SHARED" }
    without_binding: Text { secret: false }
    empty_binding: Text { binding: "" }
    invalid_binding: Text { binding: "BAD-NAME" }
}
config Second {
    second: Text { binding: "SHARED" secret: true default: "unsafe" }
}
"#;
        let parsed = parse(Path::new("bad-config.jadpo"), source);
        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let graph = build_semantic_graph(&[parsed]);
        let codes = graph
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>();
        assert!(codes.contains(&"CONFIG_MULTIPLE_DECLARATIONS"));
        assert!(codes.contains(&"CONFIG_BINDING_REQUIRED"));
        assert!(codes.contains(&"CONFIG_DUPLICATE_BINDING"));
        assert!(codes.contains(&"CONFIG_BINDING_EMPTY"));
        assert!(codes.contains(&"CONFIG_BINDING_INVALID"));
        assert!(codes.contains(&"CONFIG_SECRET_DEFAULT"));
    }

    #[test]
    fn indexes_the_application_and_closed_principal_contract() {
        let source = r#"
application TodoApplication {
    authentication {
        principal: Principal
        revocation { mode: immediate }
    }
}
principal Principal {
    user { subject: Text }
    service { subject: Text }
}
"#;
        let parsed = parse(Path::new("authentication.jadpo"), source);
        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let graph = build_semantic_graph(&[parsed]);

        assert!(graph.diagnostics.is_empty(), "{:#?}", graph.diagnostics);
        assert_eq!(
            graph
                .node("TodoApplication")
                .expect("application node")
                .kind,
            NodeKind::Application
        );
        assert_eq!(
            graph.node("Principal").expect("principal node").kind,
            NodeKind::Principal
        );
        assert_eq!(
            graph.node("Principal.user").expect("user variant").kind,
            NodeKind::PrincipalVariant
        );
        assert_eq!(
            graph
                .node("Principal.service")
                .expect("service variant")
                .kind,
            NodeKind::PrincipalVariant
        );
    }

    #[test]
    fn indexes_authentication_strategies_slots_and_validation_modes() {
        let source = r#"
application TodoApplication {
    authentication {
        principal: Principal
        revocation { mode: immediate }
    }
}
principal Principal {
    user { subject: Text }
    service { subject: Text }
}
authentication api_bearer {
    transport { bearer: authorization_header }
    validators {
        opaque_user { mode: opaque principal: user }
        service_key { mode: api_key principal: service }
    }
}
"#;
        let parsed = parse(Path::new("authentication-strategy.jadpo"), source);
        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let graph = build_semantic_graph(&[parsed]);
        assert!(graph.diagnostics.is_empty(), "{:#?}", graph.diagnostics);
        for (name, kind) in [
            (
                "authentication.api_bearer",
                NodeKind::AuthenticationStrategy,
            ),
            (
                "authentication.api_bearer.bearer.authorization_header",
                NodeKind::CredentialSlot,
            ),
            (
                "authentication.api_bearer.validator.opaque_user.opaque",
                NodeKind::CredentialValidation,
            ),
            (
                "authentication.api_bearer.validator.service_key.api_key",
                NodeKind::CredentialValidation,
            ),
        ] {
            assert_eq!(graph.node(name).expect(name).kind, kind);
        }
    }
}
