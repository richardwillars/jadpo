use jadpo_diagnostics::{Diagnostic, SourceSpan};
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
pub use typecheck::{check_types, InferredExpression, TypeCheckResult};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NodeId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NodeKind {
    PreludeType,
    StandardFailure,
    Type,
    Enum,
    EnumVariant,
    Entity,
    Value,
    Input,
    Output,
    Field,
    Relationship,
    PersistenceConstraint,
    Failure,
    Function,
    Action,
    Test,
    Route,
}

impl NodeKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PreludeType => "prelude_type",
            Self::StandardFailure => "standard_failure",
            Self::Type => "type",
            Self::Enum => "enum",
            Self::EnumVariant => "enum_variant",
            Self::Entity => "entity",
            Self::Value => "value",
            Self::Input => "input",
            Self::Output => "output",
            Self::Field => "field",
            Self::Relationship => "relationship",
            Self::PersistenceConstraint => "persistence_constraint",
            Self::Failure => "failure",
            Self::Function => "function",
            Self::Action => "action",
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
                | Self::Entity
                | Self::Value
                | Self::Input
                | Self::Output
                | Self::Field
        )
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticGraph {
    pub modules: Vec<SemanticModule>,
    pub nodes: Vec<SemanticNode>,
    pub refinements: Vec<RefinementEdge>,
    pub calls: Vec<CallEdge>,
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

        format!(
            "{{\"schema_version\":1,\"phase\":\"semantic\",\"modules\":[{modules}],\"nodes\":[{nodes}],\"refinements\":[{refinements}],\"calls\":[{calls}]}}"
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
    manifest.push_str(&format!(
        ",\"expression_types\":[{expressions}],\"failure_contracts\":[{contracts}],\"route_failures\":[{route_failures}]}}"
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
    Failure,
    StandardFailure,
}

#[derive(Clone, Debug)]
struct PendingReference {
    refined: Option<String>,
    target: String,
    expected: ReferenceKind,
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

pub fn build_semantic_graph(files: &[ParsedSyntax]) -> SemanticGraph {
    let mut builder = GraphBuilder::default();
    builder.add_prelude();
    builder.configure_modules(files);
    for file in files {
        builder.collect_file(file);
    }
    builder.validate_relationships(files);
    builder.finish()
}

#[derive(Default)]
struct GraphBuilder {
    nodes: BTreeMap<String, PendingNode>,
    modules: Vec<SemanticModule>,
    module_scopes: BTreeMap<String, ModuleScope>,
    references: Vec<PendingReference>,
    calls: Vec<PendingCall>,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Default)]
struct ModuleScope {
    module: String,
    imported_names: BTreeSet<String>,
}

impl GraphBuilder {
    fn add_prelude(&mut self) {
        for name in [
            "Bool", "DateTime", "Decimal", "Int", "List", "Map", "Set", "Text", "Unit", "Uuid",
        ] {
            self.add_node(PendingNode {
                kind: NodeKind::PreludeType,
                name: name.to_owned(),
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
                        Diagnostic::error(
                            "MOD_PUBLIC_REQUIRES_MODULE",
                            "module-public declarations require an explicit module header",
                        ),
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
                    Diagnostic::error(
                        "MOD_MODULE_REQUIRED",
                        "every source file must declare a module once any file opts into modules",
                    ),
                    &file.source_name,
                    file.file.range,
                ));
                continue;
            };
            let name = name_expression(&module.path);
            if let Some(previous) = files_by_module.get(&name) {
                self.diagnostics.push(with_span(
                    Diagnostic::error(
                        "MOD_DUPLICATE_MODULE",
                        format!("module `{name}` is already declared"),
                    )
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
                if target_name == module_name {
                    self.diagnostics.push(with_span(
                        Diagnostic::error(
                            "MOD_SELF_IMPORT",
                            format!("module `{module_name}` cannot import itself"),
                        ),
                        &file.source_name,
                        import.range,
                    ));
                    continue;
                }
                let Some(target) = files_by_module.get(&target_name).copied() else {
                    self.diagnostics.push(with_span(
                        Diagnostic::error(
                            "MOD_UNKNOWN_MODULE",
                            format!("unknown imported module `{target_name}`"),
                        ),
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
                            Diagnostic::error(
                                "MOD_IMPORT_CONFLICT",
                                format!(
                                    "imported name `{}` conflicts with a declaration in module `{module_name}`",
                                    name.text
                                ),
                            ),
                            &file.source_name,
                            name.range,
                        ));
                        continue;
                    }
                    if !imported_names.insert(name.text.clone()) {
                        self.diagnostics.push(with_span(
                            Diagnostic::error(
                                "MOD_DUPLICATE_IMPORT",
                                format!("name `{}` is imported more than once", name.text),
                            ),
                            &file.source_name,
                            name.range,
                        ));
                        continue;
                    }
                    if !target_exports.contains(name.text.as_str()) {
                        let (code, message) = if target_declarations.contains(name.text.as_str()) {
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
                            Diagnostic::error(code, message),
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
            self.module_scopes.insert(
                file.source_name.clone(),
                ModuleScope {
                    module: module_name,
                    imported_names,
                },
            );
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
                        Diagnostic::error(
                            "MOD_IMPORT_CYCLE",
                            format!("module import cycle involving `{module_name}`"),
                        ),
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
                                let diagnostic = Diagnostic::error(
                                    "DATA_MODIFIER_NON_ENTITY",
                                    "persistence modifiers and relationships are valid only on entity fields",
                                );
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
                    let mut identity_field = None;
                    for field in &declaration.fields {
                        if declaration.kind != RecordKind::Entity
                            && (!field.persistence.is_empty() || field.reference.is_some())
                        {
                            let diagnostic = Diagnostic::error(
                                "DATA_MODIFIER_NON_ENTITY",
                                "persistence modifiers and relationships are valid only on entity fields",
                            );
                            self.diagnostics.push(with_span(
                                diagnostic,
                                &file.source_name,
                                field.range,
                            ));
                        }
                        if field.persistence.contains(&PersistenceModifier::Identity) {
                            if field.field_type.nullable {
                                let diagnostic = Diagnostic::error(
                                    "DATA_IDENTITY_NULLABLE",
                                    "an entity identity field cannot be nullable",
                                );
                                self.diagnostics.push(with_span(
                                    diagnostic,
                                    &file.source_name,
                                    field.range,
                                ));
                            }
                            if let Some(previous) = identity_field {
                                let diagnostic = Diagnostic::error(
                                    "DATA_MULTIPLE_IDENTITIES",
                                    format!(
                                        "entity `{}` already declares identity field `{previous}`",
                                        declaration.name.text
                                    ),
                                );
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
                        );
                        if let Some(reference) = &field.reference {
                            self.add_type_reference(None, &reference.target, &file.source_name);
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
                            source: file.source_name.clone(),
                            range: inverse.target.range,
                        });
                        self.add_type_reference(None, &inverse.via, &file.source_name);
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
                                Diagnostic::error(
                                    "DATA_COMPOUND_CONSTRAINT_FIELDS",
                                    "a compound persistence constraint requires at least two fields",
                                ),
                                &file.source_name,
                                constraint.range,
                            ));
                        }
                        let mut seen_fields = BTreeSet::new();
                        for field_name in &constraint.fields {
                            if !seen_fields.insert(field_name.text.as_str()) {
                                self.diagnostics.push(with_span(
                                    Diagnostic::error(
                                        "DATA_CONSTRAINT_DUPLICATE_FIELD",
                                        format!(
                                            "constraint `{}` repeats field `{}`",
                                            constraint.name.text, field_name.text
                                        ),
                                    ),
                                    &file.source_name,
                                    field_name.range,
                                ));
                                continue;
                            }
                            let Some(field) = fields.get(field_name.text.as_str()) else {
                                self.diagnostics.push(with_span(
                                    Diagnostic::error(
                                        "DATA_CONSTRAINT_UNKNOWN_FIELD",
                                        format!(
                                            "constraint `{}` names unknown field `{}.{}`",
                                            constraint.name.text,
                                            declaration.name.text,
                                            field_name.text
                                        ),
                                    ),
                                    &file.source_name,
                                    field_name.range,
                                ));
                                continue;
                            };
                            if field.field_type.nullable {
                                self.diagnostics.push(with_span(
                                    Diagnostic::error(
                                        "DATA_CONSTRAINT_NULLABLE_FIELD",
                                        "compound unique constraints require non-nullable fields for cross-adapter semantics",
                                    ),
                                    &file.source_name,
                                    field_name.range,
                                ));
                            }
                        }
                        let mut shape = constraint
                            .fields
                            .iter()
                            .map(|field| field.text.as_str())
                            .collect::<Vec<_>>();
                        shape.sort_unstable();
                        if !constraint_shapes.insert(shape) {
                            self.diagnostics.push(with_span(
                                Diagnostic::error(
                                    "DATA_DUPLICATE_CONSTRAINT_SHAPE",
                                    "another compound constraint already covers the same fields",
                                ),
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
                            );
                        }
                    }
                }
                Declaration::Callable(declaration) => {
                    let kind = match declaration.kind {
                        CallableKind::Function => NodeKind::Function,
                        CallableKind::Action => NodeKind::Action,
                    };
                    let callable_name = declaration.name.text.clone();
                    self.add_authored_node(
                        kind,
                        &callable_name,
                        &file.source_name,
                        declaration.name.range,
                    );
                    for parameter in &declaration.parameters {
                        self.add_type_reference(None, &parameter.parameter_type, &file.source_name);
                    }
                    self.add_type_reference(None, &declaration.return_type, &file.source_name);
                    for failure in &declaration.failures {
                        self.references.push(PendingReference {
                            refined: None,
                            target: failure.text.clone(),
                            expected: ReferenceKind::Failure,
                            source: file.source_name.clone(),
                            range: failure.range,
                        });
                    }
                    self.collect_block_calls(&callable_name, &declaration.body, &file.source_name);
                }
                Declaration::Test(declaration) => {
                    let name = format!("test:{}", declaration.name.text);
                    self.add_authored_node(
                        NodeKind::Test,
                        &name,
                        &file.source_name,
                        declaration.name.range,
                    );
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
                        self.add_type_reference(None, input, &file.source_name);
                    }
                    if let Some(output) = &declaration.output {
                        self.add_type_reference(None, output, &file.source_name);
                    }
                    if let Some(run) = &declaration.run {
                        self.calls.push(PendingCall {
                            caller: route_name,
                            callee: name_expression(&run.callee.path),
                            source: file.source_name.clone(),
                            range: run.callee.range,
                        });
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
                    let diagnostic = Diagnostic::error(
                        "DATA_RELATIONSHIP_TARGET_FIELD",
                        "relationship target must be a non-nullable entity field such as `User.id`",
                    );
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
                    let diagnostic = Diagnostic::error(
                        "DATA_RELATIONSHIP_TARGET_FIELD",
                        format!(
                            "entity `{target_entity_name}` has no relationship target field `{target_field_name}`"
                        ),
                    );
                    self.diagnostics
                        .push(with_span(diagnostic, source, reference.target.range));
                    continue;
                };

                if target_field.field_type.nullable
                    || (!target_field
                        .persistence
                        .contains(&PersistenceModifier::Identity)
                        && !target_field
                            .persistence
                            .contains(&PersistenceModifier::Unique))
                {
                    let diagnostic = Diagnostic::error(
                        "DATA_RELATIONSHIP_TARGET_NOT_KEY",
                        format!(
                            "relationship target `{target_entity_name}.{target_field_name}` must be a non-nullable identity or unique field"
                        ),
                    );
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
                    let diagnostic = Diagnostic::error(
                        "DATA_RELATIONSHIP_TYPE_MISMATCH",
                        format!(
                            "relationship field `{entity_name}.{}` must use nominal type `{target_entity_name}.{target_field_name}`",
                            field.name.text
                        ),
                    );
                    self.diagnostics
                        .push(with_span(diagnostic, source, field.field_type.range));
                }

                if reference.on_delete == ReferenceDeleteAction::SetNull
                    && !field.field_type.nullable
                {
                    let diagnostic = Diagnostic::error(
                        "DATA_RELATIONSHIP_SET_NULL_REQUIRED",
                        "`on_delete set_null` requires a nullable relationship field",
                    );
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
                    let diagnostic = Diagnostic::error(
                        "DATA_INVERSE_DUPLICATE_NAME",
                        format!(
                            "entity `{entity_name}` already has a field or inverse named `{}`",
                            inverse.name.text
                        ),
                    );
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
                    let diagnostic = Diagnostic::error(
                        "DATA_INVERSE_VIA_FIELD",
                        "inverse `via` must name a field on the declared child entity, such as `Todo.owner_id`",
                    );
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
                    let diagnostic = Diagnostic::error(
                        "DATA_INVERSE_VIA_FIELD",
                        format!(
                            "entity `{child_name}` has no relationship field `{child_field_name}`"
                        ),
                    );
                    self.diagnostics
                        .push(with_span(diagnostic, source, inverse.via.range));
                    continue;
                };
                let points_back = child_field.reference.as_ref().is_some_and(|reference| {
                    reference.target.path.len() == 2
                        && reference.target.path[0].text == *entity_name
                });
                if !points_back {
                    let diagnostic = Diagnostic::error(
                        "DATA_INVERSE_NOT_OWNING_REFERENCE",
                        format!(
                            "inverse `{entity_name}.{}` must use an owning reference on `{child_name}.{child_field_name}` that points back to `{entity_name}`",
                            inverse.name.text
                        ),
                    );
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
                    let diagnostic = Diagnostic::error(
                        "DATA_INVERSE_OPTIONAL_NOT_UNIQUE",
                        format!(
                            "optional inverse `{entity_name}.{}` requires `{child_name}.{child_field_name}` to be identity or unique",
                            inverse.name.text
                        ),
                    );
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
                    let diagnostic = Diagnostic::error(
                        "DATA_RELATIONSHIP_CYCLE",
                        format!(
                            "relationship dependency cycle involving entity `{entity_name}` cannot be emitted as a fresh schema"
                        ),
                    );
                    self.diagnostics
                        .push(with_span(diagnostic, source, reference.range));
                }
            }
        }
    }

    fn add_node(&mut self, node: PendingNode) {
        if let Some(previous) = self.nodes.get(&node.name) {
            let diagnostic = Diagnostic::error(
                "SEM_DUPLICATE_DECLARATION",
                format!("semantic name `{}` is already declared", node.name),
            )
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
    ) {
        self.references.push(PendingReference {
            refined,
            target: name_expression(&reference.path),
            expected: ReferenceKind::Type,
            source: source.to_owned(),
            range: reference.range,
        });
        for argument in &reference.arguments {
            self.add_type_reference(None, argument, source);
        }
    }

    fn collect_block_calls(&mut self, caller: &str, block: &Block, source: &str) {
        for statement in &block.statements {
            match statement {
                Statement::Binding(statement) => {
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
            }
            Expression::Construction(construction) => {
                for field in &construction.fields {
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
            Expression::Literal(_) | Expression::Name(_) | Expression::Missing(_) => {}
        }
    }

    fn finish(mut self) -> SemanticGraph {
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
                let diagnostic = Diagnostic::error(
                    "SEM_UNKNOWN_NAME",
                    format!("unknown semantic name `{}`", reference.target),
                );
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
                ReferenceKind::Failure => {
                    matches!(target_kind, NodeKind::Failure | NodeKind::StandardFailure)
                }
                ReferenceKind::StandardFailure => target_kind == NodeKind::StandardFailure,
            };
            if !kind_matches {
                let diagnostic = Diagnostic::error(
                    "SEM_WRONG_NAME_KIND",
                    format!(
                        "`{}` has kind `{}`, which is not valid in this position",
                        reference.target,
                        target_kind.as_str()
                    ),
                );
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
            let Some(callee) = ids.get(&call.callee).copied() else {
                let diagnostic = Diagnostic::error(
                    "SEM_UNKNOWN_CALLEE",
                    format!("unknown callable or constructor `{}`", call.callee),
                );
                self.diagnostics
                    .push(with_span(diagnostic, &call.source, call.range));
                continue;
            };
            if let Some(diagnostic) = module_visibility_diagnostic(
                &self.module_scopes,
                &nodes[callee.0 as usize],
                &call.callee,
                &call.source,
                call.range,
            ) {
                self.diagnostics.push(diagnostic);
                continue;
            }
            match kinds[&call.callee] {
                NodeKind::Function | NodeKind::Action => calls.push(CallEdge { caller, callee }),
                kind if kind.is_type() => {}
                other => {
                    let diagnostic = Diagnostic::error(
                        "SEM_NOT_CALLABLE",
                        format!(
                            "`{}` has kind `{}` and cannot be invoked",
                            call.callee,
                            other.as_str()
                        ),
                    );
                    self.diagnostics
                        .push(with_span(diagnostic, &call.source, call.range));
                }
            }
        }

        refinements.sort_by_key(|edge| (edge.refined, edge.parent));
        refinements.dedup();
        calls.sort_by_key(|edge| (edge.caller, edge.callee));
        calls.dedup();

        SemanticGraph {
            modules: self.modules,
            nodes,
            refinements,
            calls,
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
        Diagnostic::error(
            "MOD_IMPORT_REQUIRED",
            format!(
                "module `{}` must selectively import `{root_name}` before using `{target_name}`",
                scope.module
            ),
        ),
        source,
        range,
    ))
}

fn declaration_name(declaration: &Declaration) -> Option<&jadpo_syntax::Name> {
    match declaration {
        Declaration::Type(declaration) => Some(&declaration.name),
        Declaration::Enum(declaration) => Some(&declaration.name),
        Declaration::Record(declaration) => Some(&declaration.name),
        Declaration::Failure(declaration) => Some(&declaration.name),
        Declaration::Callable(declaration) => Some(&declaration.name),
        Declaration::Test(_) => None,
        Declaration::Route(_) => None,
    }
}

fn name_expression(path: &[jadpo_syntax::Name]) -> String {
    path.iter()
        .map(|name| name.text.as_str())
        .collect::<Vec<_>>()
        .join(".")
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
type Email = Text { format email }
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
            "module todo.shared\npublic type Email = Text { format email }\n",
        );
        let api = parse(
            Path::new("api.jadpo"),
            "module todo.api\nimport todo.shared { Email }\npublic input Signup { email: Email }\n",
        );
        assert!(shared.diagnostics.is_empty(), "{:#?}", shared.diagnostics);
        assert!(api.diagnostics.is_empty(), "{:#?}", api.diagnostics);
        let graph = build_semantic_graph(&[shared.clone(), api]);
        assert!(graph.diagnostics.is_empty(), "{:#?}", graph.diagnostics);
        assert_eq!(graph.modules.len(), 2);
        assert_eq!(graph.modules[0].name, "todo.api");
        assert_eq!(graph.modules[0].imports, vec!["todo.shared.Email"]);
        assert!(graph
            .manifest()
            .to_json()
            .contains("\"modules\":[{\"name\":\"todo.api\""));

        let missing_import = parse(
            Path::new("missing.jadpo"),
            "module todo.consumer\ninput SignupWithoutImport { email: Email }\n",
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
}
