use crate::{NodeKind, SemanticGraph};
use jadpo_diagnostics::{Diagnostic, SourceSpan};
use jadpo_syntax::{
    Block, CallableDeclaration, Constraint, ConstraintKind, Declaration, Expression,
    FieldInitialiser, InvocationExpression, Literal, LiteralKind, Name, ParsedSyntax,
    PersistenceModifier, Statement, TextRange, TypeReference,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InferredExpression {
    pub source: String,
    pub range: TextRange,
    pub type_name: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TypeCheckResult {
    pub diagnostics: Vec<Diagnostic>,
    pub expressions: Vec<InferredExpression>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TypeValue {
    name: String,
    arguments: Vec<TypeValue>,
    nullable: bool,
}

impl TypeValue {
    fn display(&self) -> String {
        let arguments = if self.arguments.is_empty() || self.name.contains('.') {
            String::new()
        } else {
            format!(
                "<{}>",
                self.arguments
                    .iter()
                    .map(Self::display)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        let nullable = if self.nullable { "?" } else { "" };
        format!("{}{arguments}{nullable}", self.name)
    }
}

#[derive(Clone, Debug)]
struct CallableSignature {
    parameters: Vec<TypeValue>,
    result: TypeValue,
}

#[derive(Clone, Debug)]
struct RecordField {
    declared_type: TypeValue,
    optional: bool,
}

#[derive(Clone, Debug)]
struct InverseInfo {
    child: String,
    cardinality: jadpo_syntax::InverseCardinality,
}

#[derive(Clone, Debug)]
struct OwningReferenceInfo {
    parent: String,
    nullable: bool,
}

#[derive(Default)]
struct Catalogue {
    callables: BTreeMap<String, CallableSignature>,
    records: BTreeMap<String, BTreeMap<String, RecordField>>,
    failure_fields: BTreeMap<String, BTreeMap<String, RecordField>>,
    constraints: BTreeMap<String, Vec<Constraint>>,
    entities: BTreeSet<String>,
    ordered_keys: BTreeMap<String, BTreeSet<String>>,
    persistence_constraints: BTreeSet<String>,
    inverses: BTreeMap<String, BTreeMap<String, InverseInfo>>,
    owning_references: BTreeMap<String, BTreeMap<String, OwningReferenceInfo>>,
    record_kinds: BTreeMap<String, jadpo_syntax::RecordKind>,
    enums: BTreeMap<String, BTreeMap<String, BTreeMap<String, RecordField>>>,
}

pub fn check_types(files: &[ParsedSyntax], graph: &SemanticGraph) -> TypeCheckResult {
    let catalogue = Catalogue::from_files(files);
    let mut checker = TypeChecker {
        graph,
        catalogue,
        result: TypeCheckResult::default(),
    };
    checker.check_files(files);
    checker.result
}

impl Catalogue {
    fn from_files(files: &[ParsedSyntax]) -> Self {
        let mut catalogue = Self::default();
        for file in files {
            for declaration in &file.file.declarations {
                match declaration {
                    Declaration::Type(declaration) => {
                        catalogue.constraints.insert(
                            declaration.name.text.clone(),
                            declaration.constraints.clone(),
                        );
                    }
                    Declaration::Enum(declaration) => {
                        let mut variants = BTreeMap::new();
                        for variant in &declaration.variants {
                            let mut fields = BTreeMap::new();
                            for field in &variant.fields {
                                catalogue.constraints.insert(
                                    format!(
                                        "{}.{}.{}",
                                        declaration.name.text, variant.name.text, field.name.text
                                    ),
                                    field.constraints.clone(),
                                );
                                fields.insert(
                                    field.name.text.clone(),
                                    RecordField {
                                        declared_type: type_value(&field.field_type),
                                        optional: field.optional,
                                    },
                                );
                            }
                            variants.insert(variant.name.text.clone(), fields);
                        }
                        catalogue
                            .enums
                            .insert(declaration.name.text.clone(), variants);
                    }
                    Declaration::Record(declaration) => {
                        catalogue
                            .record_kinds
                            .insert(declaration.name.text.clone(), declaration.kind);
                        if declaration.kind == jadpo_syntax::RecordKind::Entity {
                            catalogue.entities.insert(declaration.name.text.clone());
                            catalogue.ordered_keys.insert(
                                declaration.name.text.clone(),
                                declaration
                                    .fields
                                    .iter()
                                    .filter(|field| {
                                        field.persistence.contains(&PersistenceModifier::Identity)
                                            || field
                                                .persistence
                                                .contains(&PersistenceModifier::Unique)
                                    })
                                    .map(|field| field.name.text.clone())
                                    .collect(),
                            );
                            catalogue.persistence_constraints.extend(
                                declaration
                                    .fields
                                    .iter()
                                    .filter(|field| {
                                        field.persistence.contains(&PersistenceModifier::Identity)
                                            || field
                                                .persistence
                                                .contains(&PersistenceModifier::Unique)
                                    })
                                    .map(|field| {
                                        format!("{}.{}", declaration.name.text, field.name.text)
                                    }),
                            );
                            catalogue.persistence_constraints.extend(
                                declaration
                                    .persistence_constraints
                                    .iter()
                                    .map(|constraint| {
                                        format!(
                                            "{}.{}",
                                            declaration.name.text, constraint.name.text
                                        )
                                    }),
                            );
                        }
                        let fields = declaration
                            .fields
                            .iter()
                            .map(|field| {
                                let full_name =
                                    format!("{}.{}", declaration.name.text, field.name.text);
                                catalogue
                                    .constraints
                                    .insert(full_name, field.constraints.clone());
                                (
                                    field.name.text.clone(),
                                    RecordField {
                                        declared_type: type_value(&field.field_type),
                                        optional: field.optional,
                                    },
                                )
                            })
                            .collect();
                        catalogue
                            .records
                            .insert(declaration.name.text.clone(), fields);
                        catalogue.inverses.insert(
                            declaration.name.text.clone(),
                            declaration
                                .inverses
                                .iter()
                                .map(|inverse| {
                                    (
                                        inverse.name.text.clone(),
                                        InverseInfo {
                                            child: inverse.target.text.clone(),
                                            cardinality: inverse.cardinality,
                                        },
                                    )
                                })
                                .collect(),
                        );
                        catalogue.owning_references.insert(
                            declaration.name.text.clone(),
                            declaration
                                .fields
                                .iter()
                                .filter_map(|field| {
                                    let reference = field.reference.as_ref()?;
                                    Some((
                                        reference.relationship.as_ref().map_or_else(
                                            || field.name.text.clone(),
                                            |name| name.text.clone(),
                                        ),
                                        OwningReferenceInfo {
                                            parent: reference.target.path[0].text.clone(),
                                            nullable: field.field_type.nullable,
                                        },
                                    ))
                                })
                                .collect(),
                        );
                    }
                    Declaration::Failure(declaration) => {
                        for (scope, fields) in [
                            ("public", &declaration.public_fields),
                            ("internal", &declaration.internal_fields),
                        ] {
                            catalogue.failure_fields.insert(
                                format!("{}.{}", declaration.name.text, scope),
                                fields
                                    .iter()
                                    .map(|field| {
                                        (
                                            field.name.text.clone(),
                                            RecordField {
                                                declared_type: type_value(&field.field_type),
                                                optional: field.optional,
                                            },
                                        )
                                    })
                                    .collect(),
                            );
                            for field in fields {
                                catalogue.constraints.insert(
                                    format!(
                                        "{}.{}.{}",
                                        declaration.name.text, scope, field.name.text
                                    ),
                                    field.constraints.clone(),
                                );
                            }
                        }
                    }
                    Declaration::Callable(declaration) => {
                        catalogue.callables.insert(
                            declaration.name.text.clone(),
                            CallableSignature {
                                parameters: declaration
                                    .parameters
                                    .iter()
                                    .map(|parameter| type_value(&parameter.parameter_type))
                                    .collect(),
                                result: type_value(&declaration.return_type),
                            },
                        );
                    }
                    Declaration::Test(_) | Declaration::Route(_) => {}
                }
            }
        }
        catalogue
    }
}

struct TypeChecker<'graph> {
    graph: &'graph SemanticGraph,
    catalogue: Catalogue,
    result: TypeCheckResult,
}

impl TypeChecker<'_> {
    fn check_files(&mut self, files: &[ParsedSyntax]) {
        for file in files {
            for declaration in &file.file.declarations {
                match declaration {
                    Declaration::Callable(callable) => {
                        self.check_callable(callable, &file.source_name)
                    }
                    Declaration::Route(route) => {
                        if let Some(run) = &route.run {
                            let mut environment = BTreeMap::new();
                            if let Some(input) = &route.input {
                                environment.insert("input".to_owned(), type_value(input));
                            }
                            self.infer_invocation(run, &environment, &file.source_name);
                        }
                    }
                    Declaration::Test(test) => {
                        let mut environment = BTreeMap::new();
                        let mut mutable_bindings = BTreeSet::new();
                        self.check_block(
                            &test.body,
                            &mut environment,
                            &mut mutable_bindings,
                            &simple_type("Unit"),
                            &file.source_name,
                        );
                    }
                    Declaration::Type(_)
                    | Declaration::Enum(_)
                    | Declaration::Record(_)
                    | Declaration::Failure(_) => {}
                }
            }
        }
    }

    fn check_callable(&mut self, callable: &CallableDeclaration, source: &str) {
        let mut environment = BTreeMap::new();
        let mut mutable_bindings = BTreeSet::new();
        for parameter in &callable.parameters {
            let parameter_type = type_value(&parameter.parameter_type);
            if self.is_primitive_signature_type(&parameter_type) {
                self.push_diagnostic(
                    "TYPE_PRIMITIVE_SIGNATURE",
                    format!(
                        "parameter `{}` uses primitive type `{}`; declare an application type",
                        parameter.name.text,
                        parameter_type.display()
                    ),
                    source,
                    parameter.range,
                );
            }
            environment.insert(parameter.name.text.clone(), parameter_type);
        }

        let return_type = type_value(&callable.return_type);
        if self.is_primitive_signature_type(&return_type) {
            self.push_diagnostic(
                "TYPE_PRIMITIVE_SIGNATURE",
                format!(
                    "return position uses primitive type `{}`; declare an application type",
                    return_type.display()
                ),
                source,
                callable.return_annotation_range,
            );
        }

        self.check_block(
            &callable.body,
            &mut environment,
            &mut mutable_bindings,
            &return_type,
            source,
        );
    }

    fn check_block(
        &mut self,
        block: &Block,
        environment: &mut BTreeMap<String, TypeValue>,
        mutable_bindings: &mut BTreeSet<String>,
        return_type: &TypeValue,
        source: &str,
    ) {
        for statement in &block.statements {
            match statement {
                Statement::Binding(binding) => {
                    let inferred = self.infer_expression(&binding.value, environment, source);
                    let binding_type = if let Some(annotation) = &binding.annotation {
                        let expected = type_value(annotation);
                        if let Some(received) = &inferred {
                            self.require_compatible(
                                received,
                                &expected,
                                source,
                                binding.value.range(),
                            );
                        }
                        expected
                    } else if let Some(inferred) = inferred {
                        inferred
                    } else {
                        continue;
                    };
                    environment.insert(binding.name.text.clone(), binding_type);
                    if binding.mutable {
                        mutable_bindings.insert(binding.name.text.clone());
                    } else {
                        mutable_bindings.remove(&binding.name.text);
                    }
                }
                Statement::Assignment(assignment) => {
                    let expected = environment.get(&assignment.target.text).cloned();
                    if expected.is_none() {
                        self.push_diagnostic(
                            "TYPE_ASSIGN_UNKNOWN",
                            format!(
                                "cannot assign to unknown local `{}`",
                                assignment.target.text
                            ),
                            source,
                            assignment.range,
                        );
                    } else if !mutable_bindings.contains(&assignment.target.text) {
                        self.push_diagnostic(
                            "TYPE_ASSIGN_IMMUTABLE",
                            format!(
                                "cannot assign to immutable value `{}`; only locals declared with `var mut` may be reassigned",
                                assignment.target.text
                            ),
                            source,
                            assignment.range,
                        );
                    }
                    if let (Some(received), Some(expected)) = (
                        self.infer_expression(&assignment.value, environment, source),
                        expected,
                    ) {
                        self.require_compatible(
                            &received,
                            &expected,
                            source,
                            assignment.value.range(),
                        );
                    }
                }
                Statement::Return(statement) => {
                    if let Some(received) =
                        self.infer_expression(&statement.value, environment, source)
                    {
                        self.require_compatible(
                            &received,
                            return_type,
                            source,
                            statement.value.range(),
                        );
                    }
                }
                Statement::Reject(statement) => {
                    self.check_reject_fields(
                        &statement.failure.text,
                        "public",
                        &statement.public_values,
                        environment,
                        source,
                    );
                    self.check_reject_fields(
                        &statement.failure.text,
                        "internal",
                        &statement.internal_values,
                        environment,
                        source,
                    );
                }
                Statement::If(statement) => {
                    if let Some(condition) =
                        self.infer_expression(&statement.condition, environment, source)
                    {
                        self.require_compatible(
                            &condition,
                            &simple_type("Bool"),
                            source,
                            statement.condition.range(),
                        );
                    }
                    let mut then_environment = environment.clone();
                    let mut then_mutable_bindings = mutable_bindings.clone();
                    self.check_block(
                        &statement.then_block,
                        &mut then_environment,
                        &mut then_mutable_bindings,
                        return_type,
                        source,
                    );
                    if let Some(else_block) = &statement.else_block {
                        let mut else_environment = environment.clone();
                        let mut else_mutable_bindings = mutable_bindings.clone();
                        self.check_block(
                            else_block,
                            &mut else_environment,
                            &mut else_mutable_bindings,
                            return_type,
                            source,
                        );
                    }
                }
                Statement::Match(statement) => {
                    self.check_match_statement(
                        statement,
                        environment,
                        mutable_bindings,
                        return_type,
                        source,
                    );
                }
                Statement::Assert(statement) => {
                    if let Some(condition) =
                        self.infer_expression(&statement.condition, environment, source)
                    {
                        self.require_compatible(
                            &condition,
                            &simple_type("Bool"),
                            source,
                            statement.condition.range(),
                        );
                    }
                }
                Statement::Unsupported(_) => {}
            }
        }
    }

    fn check_match_statement(
        &mut self,
        statement: &jadpo_syntax::MatchStatement,
        environment: &BTreeMap<String, TypeValue>,
        mutable_bindings: &BTreeSet<String>,
        return_type: &TypeValue,
        source: &str,
    ) {
        let subject_type = self.infer_expression(&statement.subject, environment, source);
        let enum_name = subject_type
            .as_ref()
            .and_then(|subject| self.enum_shape_name(&subject.name));
        let mut seen = BTreeSet::new();
        let mut wildcard_seen = false;
        let mut some_seen = false;

        for arm in &statement.arms {
            let mut pattern_bindings = Vec::new();
            if wildcard_seen {
                self.push_diagnostic(
                    "TYPE_MATCH_UNREACHABLE_PATTERN",
                    "match patterns after `_` are unreachable",
                    source,
                    arm.pattern.range(),
                );
            }
            match &arm.pattern {
                jadpo_syntax::MatchPattern::Wildcard(_) => {
                    if !seen.insert("_".to_owned()) {
                        self.push_diagnostic(
                            "TYPE_MATCH_DUPLICATE_PATTERN",
                            "wildcard pattern `_` appears more than once",
                            source,
                            arm.pattern.range(),
                        );
                    }
                    wildcard_seen = true;
                }
                jadpo_syntax::MatchPattern::Name(pattern) => {
                    let pattern_name = joined_name(&pattern.path);
                    if let Some(expected_enum) = enum_name.as_deref() {
                        let expected_prefix = format!("{expected_enum}.");
                        if let Some(variant) = pattern_name.strip_prefix(&expected_prefix) {
                            if variant.contains('.')
                                || !self
                                    .catalogue
                                    .enums
                                    .get(expected_enum)
                                    .is_some_and(|variants| variants.contains_key(variant))
                            {
                                self.push_diagnostic(
                                    "TYPE_MATCH_UNKNOWN_VARIANT",
                                    format!(
                                        "`{pattern_name}` is not a variant of `{expected_enum}`"
                                    ),
                                    source,
                                    pattern.range,
                                );
                            } else if !seen.insert(variant.to_owned()) {
                                self.push_diagnostic(
                                    "TYPE_MATCH_DUPLICATE_PATTERN",
                                    format!("variant `{pattern_name}` is matched more than once"),
                                    source,
                                    pattern.range,
                                );
                            } else if self
                                .catalogue
                                .enums
                                .get(expected_enum)
                                .and_then(|variants| variants.get(variant))
                                .is_some_and(|fields| !fields.is_empty())
                            {
                                self.push_diagnostic(
                                    "TYPE_MATCH_VARIANT_BINDINGS_REQUIRED",
                                    format!(
                                        "payload variant `{pattern_name}` requires a `{{ ... }}` pattern"
                                    ),
                                    source,
                                    pattern.range,
                                );
                            }
                        } else {
                            self.push_diagnostic(
                                "TYPE_MATCH_PATTERN_TYPE",
                                format!(
                                    "match over `{expected_enum}` requires `{expected_enum}.variant` patterns"
                                ),
                                source,
                                pattern.range,
                            );
                        }
                    } else {
                        self.push_diagnostic(
                            "TYPE_MATCH_PATTERN_TYPE",
                            "named match patterns require an enum subject",
                            source,
                            pattern.range,
                        );
                    }
                }
                jadpo_syntax::MatchPattern::Variant(pattern) => {
                    let pattern_name = joined_name(&pattern.target.path);
                    if let Some(expected_enum) = enum_name.as_deref() {
                        let expected_prefix = format!("{expected_enum}.");
                        if let Some(variant) = pattern_name.strip_prefix(&expected_prefix) {
                            let fields = self
                                .catalogue
                                .enums
                                .get(expected_enum)
                                .and_then(|variants| variants.get(variant))
                                .cloned();
                            if variant.contains('.') || fields.is_none() {
                                self.push_diagnostic(
                                    "TYPE_MATCH_UNKNOWN_VARIANT",
                                    format!(
                                        "`{pattern_name}` is not a variant of `{expected_enum}`"
                                    ),
                                    source,
                                    pattern.target.range,
                                );
                            } else {
                                if !seen.insert(variant.to_owned()) {
                                    self.push_diagnostic(
                                        "TYPE_MATCH_DUPLICATE_PATTERN",
                                        format!(
                                            "variant `{pattern_name}` is matched more than once"
                                        ),
                                        source,
                                        pattern.target.range,
                                    );
                                }
                                let fields = fields.unwrap_or_default();
                                let mut bound = BTreeSet::new();
                                for binding in &pattern.bindings {
                                    if !bound.insert(binding.text.clone()) {
                                        self.push_diagnostic(
                                            "TYPE_MATCH_DUPLICATE_BINDING",
                                            format!(
                                                "variant field `{}` is bound more than once",
                                                binding.text
                                            ),
                                            source,
                                            binding.range,
                                        );
                                    } else if let Some(field) = fields.get(&binding.text) {
                                        pattern_bindings.push((
                                            binding.text.clone(),
                                            TypeValue {
                                                name: format!(
                                                    "{expected_enum}.{variant}.{}",
                                                    binding.text
                                                ),
                                                arguments: field.declared_type.arguments.clone(),
                                                nullable: field.declared_type.nullable,
                                            },
                                        ));
                                    } else {
                                        self.push_diagnostic(
                                            "TYPE_MATCH_UNKNOWN_BINDING",
                                            format!(
                                                "variant `{pattern_name}` has no field `{}`",
                                                binding.text
                                            ),
                                            source,
                                            binding.range,
                                        );
                                    }
                                }
                            }
                        } else {
                            self.push_diagnostic(
                                "TYPE_MATCH_PATTERN_TYPE",
                                format!(
                                    "match over `{expected_enum}` requires `{expected_enum}.variant` patterns"
                                ),
                                source,
                                pattern.target.range,
                            );
                        }
                    } else {
                        self.push_diagnostic(
                            "TYPE_MATCH_PATTERN_TYPE",
                            "variant match patterns require an enum subject",
                            source,
                            pattern.target.range,
                        );
                    }
                }
                jadpo_syntax::MatchPattern::OptionalSome(pattern) => {
                    let Some(subject) = subject_type.as_ref() else {
                        continue;
                    };
                    if !subject.nullable {
                        self.push_diagnostic(
                            "TYPE_MATCH_SOME_NON_OPTIONAL",
                            "`some(...)` requires a nullable match subject",
                            source,
                            pattern.range,
                        );
                    }
                    if some_seen {
                        self.push_diagnostic(
                            "TYPE_MATCH_DUPLICATE_PATTERN",
                            "`some(...)` appears more than once",
                            source,
                            pattern.range,
                        );
                    }
                    some_seen = true;
                    pattern_bindings.push((
                        pattern.binding.text.clone(),
                        TypeValue {
                            nullable: false,
                            ..subject.clone()
                        },
                    ));
                }
                jadpo_syntax::MatchPattern::Literal(pattern) => {
                    if let (Some(subject), Some(pattern_type)) =
                        (subject_type.as_ref(), self.literal_type(pattern))
                    {
                        let root = self.representation_root(&subject.name);
                        let nullable_none = pattern_type.name == "none" && subject.nullable;
                        if !nullable_none && root.as_deref() != Some(pattern_type.name.as_str()) {
                            self.push_diagnostic(
                                "TYPE_MATCH_PATTERN_TYPE",
                                format!(
                                    "literal pattern has type `{}`, but match subject has type `{}`",
                                    pattern_type.display(),
                                    subject.display()
                                ),
                                source,
                                pattern.range,
                            );
                        }
                    }
                    if !seen.insert(pattern.text.clone()) {
                        self.push_diagnostic(
                            "TYPE_MATCH_DUPLICATE_PATTERN",
                            format!("literal `{}` is matched more than once", pattern.text),
                            source,
                            pattern.range,
                        );
                    }
                }
            }

            let mut arm_environment = environment.clone();
            arm_environment.extend(pattern_bindings);
            let mut arm_mutable_bindings = mutable_bindings.clone();
            self.check_block(
                &arm.body,
                &mut arm_environment,
                &mut arm_mutable_bindings,
                return_type,
                source,
            );
        }

        if wildcard_seen {
            return;
        }
        if let Some(enum_name) = enum_name {
            let mut missing = self
                .catalogue
                .enums
                .get(&enum_name)
                .into_iter()
                .flat_map(|variants| variants.keys())
                .filter(|variant| !some_seen && !seen.contains(*variant))
                .cloned()
                .collect::<Vec<_>>();
            if subject_type
                .as_ref()
                .is_some_and(|subject| subject.nullable)
                && !seen.contains("none")
            {
                missing.push("none".to_owned());
            }
            if !missing.is_empty() {
                self.push_diagnostic(
                    "TYPE_MATCH_NON_EXHAUSTIVE",
                    format!(
                        "match over `{enum_name}` is missing: {}",
                        missing
                            .iter()
                            .map(|variant| if variant == "none" {
                                "none".to_owned()
                            } else {
                                format!("{enum_name}.{variant}")
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    source,
                    statement.range,
                );
            }
        } else if let Some(subject) = subject_type {
            if self.representation_root(&subject.name).as_deref() == Some("Bool") {
                let mut missing = ["true", "false"]
                    .into_iter()
                    .filter(|value| !some_seen && !seen.contains(*value))
                    .collect::<Vec<_>>();
                if subject.nullable && !seen.contains("none") {
                    missing.push("none");
                }
                if !missing.is_empty() {
                    self.push_diagnostic(
                        "TYPE_MATCH_NON_EXHAUSTIVE",
                        format!("match over `Bool` is missing: {}", missing.join(", ")),
                        source,
                        statement.range,
                    );
                }
            } else if !(subject.nullable && some_seen && seen.contains("none")) {
                self.push_diagnostic(
                    "TYPE_MATCH_WILDCARD_REQUIRED",
                    "match over an open value requires a final `_` arm",
                    source,
                    statement.range,
                );
            }
        }
    }

    fn infer_expression(
        &mut self,
        expression: &Expression,
        environment: &BTreeMap<String, TypeValue>,
        source: &str,
    ) -> Option<TypeValue> {
        let inferred = match expression {
            Expression::Literal(literal) => self.literal_type(literal),
            Expression::Name(name) => self.resolve_value_path(&name.path, environment, source),
            Expression::Invocation(invocation) => {
                self.infer_invocation(invocation, environment, source)
            }
            Expression::Construction(construction) => {
                let target = joined_name(&construction.target.path);
                if let Some((enum_name, _variant, fields)) = self.enum_variant(&target) {
                    self.check_variant_construction(
                        &target,
                        &fields,
                        &construction.fields,
                        construction.range,
                        environment,
                        source,
                    );
                    Some(simple_type(&enum_name))
                } else {
                    self.check_construction(
                        &target,
                        &construction.fields,
                        construction.range,
                        environment,
                        source,
                        false,
                    );
                    Some(simple_type(&target))
                }
            }
            Expression::Create(create) => {
                let target = joined_name(&create.target.path);
                if !self.catalogue.entities.contains(&target) {
                    self.push_diagnostic(
                        "TYPE_CREATE_NOT_ENTITY",
                        format!("`create` requires an entity, but `{target}` is not persistent"),
                        source,
                        create.target.range,
                    );
                }
                self.check_construction(
                    &target,
                    &create.fields,
                    create.range,
                    environment,
                    source,
                    true,
                );
                self.check_conflict_bindings(&target, &create.conflicts, environment, source);
                Some(simple_type(&target))
            }
            Expression::Query(query) => {
                let target = joined_name(&query.target.path);
                if !self.catalogue.entities.contains(&target) {
                    self.push_diagnostic(
                        "TYPE_QUERY_NOT_ENTITY",
                        format!("`query` requires an entity, but `{target}` is not persistent"),
                        source,
                        query.target.range,
                    );
                }
                let expected = self
                    .catalogue
                    .records
                    .get(&target)
                    .and_then(|fields| fields.get(&query.field.text))
                    .cloned();
                if let Some(expected) = expected {
                    if expected.declared_type.nullable {
                        self.push_diagnostic(
                            "TYPE_QUERY_NULLABLE_FIELD_UNSUPPORTED",
                            format!(
                                "query predicates cannot yet target nullable field `{target}.{}`",
                                query.field.text
                            ),
                            source,
                            query.field.range,
                        );
                    }
                    if let Some(received) = self.infer_expression(&query.value, environment, source)
                    {
                        let field_type = TypeValue {
                            name: format!("{target}.{}", query.field.text),
                            arguments: expected.declared_type.arguments,
                            nullable: expected.declared_type.nullable,
                        };
                        self.require_compatible(
                            &received,
                            &field_type,
                            source,
                            query.value.range(),
                        );
                    }
                } else {
                    if self.catalogue.records.contains_key(&target) {
                        self.push_diagnostic(
                            "TYPE_QUERY_UNKNOWN_FIELD",
                            format!("entity `{target}` has no field `{}`", query.field.text),
                            source,
                            query.field.range,
                        );
                    }
                    self.infer_expression(&query.value, environment, source);
                }
                if let Some(missing) = &query.missing {
                    self.check_reject_fields(
                        &missing.failure.text,
                        "public",
                        &missing.public_values,
                        environment,
                        source,
                    );
                    self.check_reject_fields(
                        &missing.failure.text,
                        "internal",
                        &missing.internal_values,
                        environment,
                        source,
                    );
                }
                if let Some(order) = &query.order {
                    if !self
                        .catalogue
                        .records
                        .get(&target)
                        .is_some_and(|fields| fields.contains_key(&order.field.text))
                    {
                        self.push_diagnostic(
                            "TYPE_QUERY_UNKNOWN_ORDER_FIELD",
                            format!(
                                "entity `{target}` has no ordering field `{}`",
                                order.field.text
                            ),
                            source,
                            order.field.range,
                        );
                    } else if !self
                        .catalogue
                        .ordered_keys
                        .get(&target)
                        .is_some_and(|fields| fields.contains(&order.field.text))
                    {
                        self.push_diagnostic(
                            "TYPE_QUERY_ORDER_NOT_DETERMINISTIC",
                            format!(
                                "many-result query ordering field `{target}.{}` must be identity or unique",
                                order.field.text
                            ),
                            source,
                            order.field.range,
                        );
                    }
                }
                if let Some(pagination) = &query.pagination {
                    self.check_query_pagination(pagination, environment, source);
                }
                let nested_result = query.includes.first().and_then(|first_include| {
                    let nested_name = first_include.nested_relationship.as_ref()?;
                    let result_name = joined_name(&first_include.result.path);
                    if query.includes.len() != 1 {
                        self.push_diagnostic(
                            "TYPE_NESTED_INCLUDE_SINGLE",
                            "a nested include must be the only relationship in the bounded depth-two slice",
                            source,
                            first_include.range,
                        );
                    }
                    if query.cardinality != jadpo_syntax::QueryCardinality::Required {
                        self.push_diagnostic(
                            "TYPE_NESTED_INCLUDE_REQUIRED_QUERY",
                            "a nested include currently requires a `query required` root",
                            source,
                            first_include.range,
                        );
                    }
                    if first_include.cardinality
                        != jadpo_syntax::QueryIncludeCardinality::Optional
                    {
                        self.push_diagnostic(
                            "TYPE_NESTED_INCLUDE_CARDINALITY",
                            "the second hop in the first nested slice must be an optional inverse",
                            source,
                            first_include.range,
                        );
                    }
                    let reference = self
                        .catalogue
                        .owning_references
                        .get(&target)
                        .and_then(|references| {
                            references.get(&first_include.relationship.text)
                        })
                        .cloned();
                    let Some(reference) = reference else {
                        self.push_diagnostic(
                            "TYPE_NESTED_INCLUDE_FIRST_HOP",
                            format!(
                                "entity `{target}` has no owning reference `{}` for the first nested hop",
                                first_include.relationship.text
                            ),
                            source,
                            first_include.relationship.range,
                        );
                        return Some(simple_type(&result_name));
                    };
                    if reference.nullable {
                        self.push_diagnostic(
                            "TYPE_NESTED_INCLUDE_NULLABLE_FIRST_HOP",
                            "the first nested hop must be a non-nullable owning reference",
                            source,
                            first_include.relationship.range,
                        );
                    }
                    let nested_inverse = self
                        .catalogue
                        .inverses
                        .get(&reference.parent)
                        .and_then(|relationships| relationships.get(&nested_name.text))
                        .filter(|inverse| {
                            inverse.cardinality
                                == jadpo_syntax::InverseCardinality::Optional
                        })
                        .cloned();
                    let Some(nested_inverse) = nested_inverse else {
                        self.push_diagnostic(
                            "TYPE_NESTED_INCLUDE_SECOND_HOP",
                            format!(
                                "entity `{}` has no optional inverse `{}` for the second nested hop",
                                reference.parent, nested_name.text
                            ),
                            source,
                            nested_name.range,
                        );
                        return Some(simple_type(&result_name));
                    };
                    if self.catalogue.record_kinds.get(&result_name)
                        != Some(&jadpo_syntax::RecordKind::Output)
                    {
                        self.push_diagnostic(
                            "TYPE_INCLUDE_RESULT_NOT_OUTPUT",
                            format!(
                                "included relationship result `{result_name}` must be an output declaration"
                            ),
                            source,
                            first_include.result.range,
                        );
                    }
                    let outer_shape = self.catalogue.records.get(&result_name);
                    let inner_type = outer_shape
                        .and_then(|fields| fields.get(&first_include.relationship.text))
                        .map(|field| field.declared_type.clone());
                    let outer_parent_ok = outer_shape
                        .and_then(|fields| fields.get("parent"))
                        .is_some_and(|field| field.declared_type == simple_type(&target));
                    let inner_name = inner_type
                        .as_ref()
                        .filter(|field_type| !field_type.nullable && field_type.arguments.is_empty())
                        .map(|field_type| field_type.name.clone());
                    let inner_output_ok = inner_name.as_ref().is_some_and(|name| {
                        self.catalogue.record_kinds.get(name)
                            == Some(&jadpo_syntax::RecordKind::Output)
                    });
                    let inner_shape = inner_name
                        .as_ref()
                        .and_then(|name| self.catalogue.records.get(name));
                    let inner_parent_ok = inner_shape
                        .and_then(|fields| fields.get("parent"))
                        .is_some_and(|field| {
                            field.declared_type == simple_type(&reference.parent)
                        });
                    let mut leaf_type = simple_type(&nested_inverse.child);
                    leaf_type.nullable = true;
                    let inner_leaf_ok = inner_shape
                        .and_then(|fields| fields.get(&nested_name.text))
                        .is_some_and(|field| field.declared_type == leaf_type);
                    if outer_shape.map_or(true, |fields| fields.len() != 2)
                        || !outer_parent_ok
                        || !inner_output_ok
                        || inner_shape.map_or(true, |fields| fields.len() != 2)
                        || !inner_parent_ok
                        || !inner_leaf_ok
                    {
                        self.push_diagnostic(
                            "TYPE_NESTED_INCLUDE_RESULT_SHAPE",
                            format!(
                                "output `{result_name}` must contain `parent: {target}` and `{}` pointing to an output with `parent: {}` and `{}: {}?`",
                                first_include.relationship.text,
                                reference.parent,
                                nested_name.text,
                                nested_inverse.child
                            ),
                            source,
                            first_include.result.range,
                        );
                    }
                    Some(simple_type(&result_name))
                });
                let inverse_one_result = query.includes.first().and_then(|first_include| {
                    if first_include.nested_relationship.is_some() {
                        return None;
                    }
                    let inverse = self
                        .catalogue
                        .inverses
                        .get(&target)
                        .and_then(|relationships| {
                            relationships.get(&first_include.relationship.text)
                        })
                        .filter(|inverse| {
                            inverse.cardinality
                                == jadpo_syntax::InverseCardinality::Optional
                        })
                        .cloned()?;
                    let result_name = joined_name(&first_include.result.path);
                    if query.includes.len() != 1 {
                        self.push_diagnostic(
                            "TYPE_INVERSE_ONE_SINGLE",
                            "an optional inverse include must be the only relationship in the first slice",
                            source,
                            first_include.range,
                        );
                    }
                    if query.cardinality != jadpo_syntax::QueryCardinality::Required {
                        self.push_diagnostic(
                            "TYPE_INVERSE_ONE_REQUIRED_QUERY",
                            "an optional inverse include currently requires a `query required` parent",
                            source,
                            first_include.range,
                        );
                    }
                    if first_include.cardinality
                        != jadpo_syntax::QueryIncludeCardinality::Optional
                    {
                        self.push_diagnostic(
                            "TYPE_INVERSE_ONE_CARDINALITY",
                            "an inverse declared `optional` must be included as `optional`",
                            source,
                            first_include.range,
                        );
                    }
                    if self.catalogue.record_kinds.get(&result_name)
                        != Some(&jadpo_syntax::RecordKind::Output)
                    {
                        self.push_diagnostic(
                            "TYPE_INCLUDE_RESULT_NOT_OUTPUT",
                            format!(
                                "included relationship result `{result_name}` must be an output declaration"
                            ),
                            source,
                            first_include.result.range,
                        );
                    }
                    let mut related_type = simple_type(&inverse.child);
                    related_type.nullable = true;
                    let shape = self.catalogue.records.get(&result_name);
                    let parent_ok = shape
                        .and_then(|fields| fields.get("parent"))
                        .is_some_and(|field| field.declared_type == simple_type(&target));
                    let related_ok = shape
                        .and_then(|fields| fields.get(&first_include.relationship.text))
                        .is_some_and(|field| field.declared_type == related_type);
                    if shape.map_or(true, |fields| fields.len() != 2)
                        || !parent_ok
                        || !related_ok
                    {
                        self.push_diagnostic(
                            "TYPE_INVERSE_ONE_RESULT_SHAPE",
                            format!(
                                "output `{result_name}` must contain exactly `parent: {target}` and `{}: {}`",
                                first_include.relationship.text,
                                related_type.display()
                            ),
                            source,
                            first_include.result.range,
                        );
                    }
                    Some(simple_type(&result_name))
                });
                let owning_result = if nested_result.is_none() && inverse_one_result.is_none() {
                    query.includes.first().and_then(|first_include| {
                    if first_include.cardinality
                        == jadpo_syntax::QueryIncludeCardinality::Many
                    {
                        return None;
                    }
                    let result_name = joined_name(&first_include.result.path);
                    if query.includes.len() != 1 {
                        self.push_diagnostic(
                            "TYPE_PARENT_INCLUDE_SINGLE",
                            "an owning-parent include must be the only relationship in the first slice",
                            source,
                            first_include.range,
                        );
                    }
                    if query.cardinality != jadpo_syntax::QueryCardinality::Required {
                        self.push_diagnostic(
                            "TYPE_PARENT_INCLUDE_REQUIRED_QUERY",
                            "an owning-parent include currently requires a `query required` child",
                            source,
                            first_include.range,
                        );
                    }
                    let reference = self
                        .catalogue
                        .owning_references
                        .get(&target)
                        .and_then(|references| {
                            references.get(&first_include.relationship.text)
                        })
                        .cloned();
                    let Some(reference) = reference else {
                        self.push_diagnostic(
                            "TYPE_PARENT_INCLUDE_UNKNOWN_REFERENCE",
                            format!(
                                "entity `{target}` has no owning reference `{}`",
                                first_include.relationship.text
                            ),
                            source,
                            first_include.relationship.range,
                        );
                        return Some(simple_type(&result_name));
                    };
                    if first_include.cardinality
                        == jadpo_syntax::QueryIncludeCardinality::Required
                        && reference.nullable
                    {
                        self.push_diagnostic(
                            "TYPE_PARENT_INCLUDE_NULLABLE_REFERENCE",
                            format!(
                                "nullable reference `{target}.{}` must be included as `optional`",
                                first_include.relationship.text
                            ),
                            source,
                            first_include.range,
                        );
                    }
                    if self.catalogue.record_kinds.get(&result_name)
                        != Some(&jadpo_syntax::RecordKind::Output)
                    {
                        self.push_diagnostic(
                            "TYPE_INCLUDE_RESULT_NOT_OUTPUT",
                            format!(
                                "included relationship result `{result_name}` must be an output declaration"
                            ),
                            source,
                            first_include.result.range,
                        );
                    }
                    let mut related_type = simple_type(&reference.parent);
                    related_type.nullable = first_include.cardinality
                        == jadpo_syntax::QueryIncludeCardinality::Optional;
                    let shape = self.catalogue.records.get(&result_name);
                    let parent_ok = shape
                        .and_then(|fields| fields.get("parent"))
                        .is_some_and(|field| field.declared_type == simple_type(&target));
                    let related_ok = shape
                        .and_then(|fields| fields.get(&first_include.relationship.text))
                        .is_some_and(|field| field.declared_type == related_type);
                    if shape.map_or(true, |fields| fields.len() != 2)
                        || !parent_ok
                        || !related_ok
                    {
                        self.push_diagnostic(
                            "TYPE_PARENT_INCLUDE_RESULT_SHAPE",
                            format!(
                                "output `{result_name}` must contain exactly `parent: {target}` and `{}: {}`",
                                first_include.relationship.text,
                                related_type.display()
                            ),
                            source,
                            first_include.result.range,
                        );
                    }
                    Some(simple_type(&result_name))
                    })
                } else {
                    None
                };
                let included_result = nested_result.or(inverse_one_result).or(owning_result).or_else(|| query.includes.first().map(|first_include| {
                    let result_name = first_include
                        .result
                        .path
                        .iter()
                        .map(|part| part.text.as_str())
                        .collect::<Vec<_>>()
                        .join(".");
                    let included_type = || {
                        if query.cardinality == jadpo_syntax::QueryCardinality::Many {
                            TypeValue {
                                name: "List".to_owned(),
                                arguments: vec![simple_type(&result_name)],
                                nullable: false,
                            }
                        } else {
                            simple_type(&result_name)
                        }
                    };
                    if query.cardinality == jadpo_syntax::QueryCardinality::Optional {
                        self.push_diagnostic(
                            "TYPE_INCLUDE_REQUIRED_PARENT",
                            "relationship includes require a `required` or paginated `many` parent query",
                            source,
                            first_include.range,
                        );
                    }
                    if query.cardinality == jadpo_syntax::QueryCardinality::Many
                        && query.pagination.is_none()
                    {
                        self.push_diagnostic(
                            "TYPE_INCLUDE_PARENT_PAGINATION_REQUIRED",
                            "a many-parent relationship load requires explicit parent `limit` and `offset`",
                            source,
                            first_include.range,
                        );
                    }
                    let mut seen_relationships = BTreeSet::new();
                    let mut included_children = Vec::new();
                    for include in &query.includes {
                        if include.cardinality
                            != jadpo_syntax::QueryIncludeCardinality::Many
                        {
                            self.push_diagnostic(
                                "TYPE_INCLUDE_MIXED_CARDINALITY",
                                "to-many inverse includes cannot be mixed with owning-parent includes",
                                source,
                                include.range,
                            );
                            continue;
                        }
                        self.check_query_pagination(
                            &include.pagination,
                            environment,
                            source,
                        );
                        let include_result_name = joined_name(&include.result.path);
                        if include_result_name != result_name {
                            self.push_diagnostic(
                                "TYPE_INCLUDE_RESULT_MISMATCH",
                                format!(
                                    "all relationships in one query must use the same output shape `{result_name}`"
                                ),
                                source,
                                include.result.range,
                            );
                        }
                        if !seen_relationships.insert(include.relationship.text.clone()) {
                            self.push_diagnostic(
                                "TYPE_INCLUDE_DUPLICATE_RELATIONSHIP",
                                format!(
                                    "relationship `{}` is included more than once",
                                    include.relationship.text
                                ),
                                source,
                                include.relationship.range,
                            );
                        }
                        let inverse = self
                            .catalogue
                            .inverses
                            .get(&target)
                            .and_then(|relationships| {
                                relationships.get(&include.relationship.text)
                            })
                            .cloned();
                        let Some(inverse) = inverse else {
                            self.push_diagnostic(
                                "TYPE_INCLUDE_UNKNOWN_RELATIONSHIP",
                                format!(
                                    "entity `{target}` has no inverse relationship `{}`",
                                    include.relationship.text
                                ),
                                source,
                                include.relationship.range,
                            );
                            continue;
                        };
                        if !self
                            .catalogue
                            .ordered_keys
                            .get(&inverse.child)
                            .is_some_and(|fields| fields.contains(&include.order.field.text))
                        {
                            self.push_diagnostic(
                                "TYPE_INCLUDE_ORDER_NOT_DETERMINISTIC",
                                format!(
                                    "included relationship ordering field `{}.{}` must be identity or unique",
                                    inverse.child, include.order.field.text
                                ),
                                source,
                                include.order.field.range,
                            );
                        }
                        included_children.push((include, inverse));
                    }
                    if self.catalogue.record_kinds.get(&result_name)
                        != Some(&jadpo_syntax::RecordKind::Output)
                    {
                        self.push_diagnostic(
                            "TYPE_INCLUDE_RESULT_NOT_OUTPUT",
                            format!("included relationship result `{result_name}` must be an output declaration"),
                            source,
                            first_include.result.range,
                        );
                    }
                    let shape = self.catalogue.records.get(&result_name);
                    let parent_ok = shape
                        .and_then(|fields| fields.get("parent"))
                        .is_some_and(|field| field.declared_type == simple_type(&target));
                    let children_ok = included_children.iter().all(|(include, inverse)| {
                        shape
                            .and_then(|fields| fields.get(&include.relationship.text))
                            .is_some_and(|field| {
                                field.declared_type
                                    == TypeValue {
                                        name: "List".to_owned(),
                                        arguments: vec![simple_type(&inverse.child)],
                                        nullable: false,
                                    }
                            })
                    });
                    if shape.map_or(true, |fields| fields.len() != query.includes.len() + 1)
                        || !parent_ok
                        || !children_ok
                    {
                        let fields = included_children
                            .iter()
                            .map(|(include, inverse)| {
                                format!("{}: List<{}>", include.relationship.text, inverse.child)
                            })
                            .collect::<Vec<_>>()
                            .join("`, `");
                        self.push_diagnostic(
                            "TYPE_INCLUDE_RESULT_SHAPE",
                            format!(
                                "output `{result_name}` must contain exactly `parent: {target}` and `{fields}` for the included relationships"
                            ),
                            source,
                            first_include.result.range,
                        );
                    }
                    included_type()
                }));
                if let Some(result) = included_result {
                    Some(result)
                } else {
                    match query.cardinality {
                        jadpo_syntax::QueryCardinality::Optional => {
                            let mut result = simple_type(&target);
                            result.nullable = true;
                            Some(result)
                        }
                        jadpo_syntax::QueryCardinality::Required => Some(simple_type(&target)),
                        jadpo_syntax::QueryCardinality::Many => Some(TypeValue {
                            name: "List".to_owned(),
                            arguments: vec![simple_type(&target)],
                            nullable: false,
                        }),
                    }
                }
            }
            Expression::Update(update) => {
                let target = joined_name(&update.target.path);
                if !self.catalogue.entities.contains(&target) {
                    self.push_diagnostic(
                        "TYPE_UPDATE_NOT_ENTITY",
                        format!("`update` requires an entity, but `{target}` is not persistent"),
                        source,
                        update.target.range,
                    );
                }
                self.check_mutation_predicate(
                    "update",
                    &target,
                    &update.field,
                    &update.value,
                    environment,
                    source,
                );
                if update.changes.is_empty() && update.patch.is_none() {
                    self.push_diagnostic(
                        "TYPE_UPDATE_FIELD_REQUIRED",
                        "an update requires at least one field in `set:` or an omission-aware `patch:` input",
                        source,
                        update.range,
                    );
                }
                let expected_fields = self.catalogue.records.get(&target).cloned();
                let mut changed_fields = BTreeSet::new();
                for change in &update.changes {
                    if !changed_fields.insert(change.name.text.clone()) {
                        self.push_diagnostic(
                            "TYPE_UPDATE_DUPLICATE_FIELD",
                            format!(
                                "update field `{target}.{}` appears more than once",
                                change.name.text
                            ),
                            source,
                            change.range,
                        );
                    }
                    let Some(expected) = expected_fields
                        .as_ref()
                        .and_then(|fields| fields.get(&change.name.text))
                    else {
                        self.push_diagnostic(
                            "TYPE_UPDATE_UNKNOWN_FIELD",
                            format!("entity `{target}` has no field `{}`", change.name.text),
                            source,
                            change.name.range,
                        );
                        continue;
                    };
                    if let Some(received) =
                        self.infer_expression(&change.value, environment, source)
                    {
                        let field_type = TypeValue {
                            name: format!("{target}.{}", change.name.text),
                            arguments: expected.declared_type.arguments.clone(),
                            nullable: expected.declared_type.nullable,
                        };
                        self.require_compatible(
                            &received,
                            &field_type,
                            source,
                            change.value.range(),
                        );
                    }
                }
                for conditional in &update.conditional_changes {
                    let change = &conditional.change;
                    if !changed_fields.insert(change.name.text.clone()) {
                        self.push_diagnostic(
                            "TYPE_UPDATE_DUPLICATE_FIELD",
                            format!(
                                "update field `{target}.{}` appears more than once",
                                change.name.text
                            ),
                            source,
                            change.range,
                        );
                    }
                    let Some(expected) = expected_fields
                        .as_ref()
                        .and_then(|fields| fields.get(&change.name.text))
                    else {
                        self.push_diagnostic(
                            "TYPE_UPDATE_UNKNOWN_FIELD",
                            format!("entity `{target}` has no field `{}`", change.name.text),
                            source,
                            change.name.range,
                        );
                        continue;
                    };
                    if let Some(received) =
                        self.infer_expression(&change.value, environment, source)
                    {
                        let field_type = TypeValue {
                            name: format!("{target}.{}", change.name.text),
                            arguments: expected.declared_type.arguments.clone(),
                            nullable: expected.declared_type.nullable,
                        };
                        self.require_compatible(
                            &received,
                            &field_type,
                            source,
                            change.value.range(),
                        );
                    }
                }
                if let Some(patch) = &update.patch {
                    if patch.path.len() != 1 {
                        self.push_diagnostic(
                            "TYPE_PATCH_INPUT_BINDING",
                            "`patch:` requires a direct input binding",
                            source,
                            patch.range,
                        );
                    }
                    let patch_expression = Expression::Name(patch.clone());
                    if let Some(received) =
                        self.infer_expression(&patch_expression, environment, source)
                    {
                        if self.catalogue.record_kinds.get(&received.name)
                            != Some(&jadpo_syntax::RecordKind::Input)
                        {
                            self.push_diagnostic(
                                "TYPE_PATCH_NOT_INPUT",
                                format!(
                                    "`patch:` requires an input record, but received `{}`",
                                    received.display()
                                ),
                                source,
                                patch.range,
                            );
                        } else if let Some(patch_fields) =
                            self.catalogue.records.get(&received.name).cloned()
                        {
                            if patch_fields.is_empty() {
                                self.push_diagnostic(
                                    "TYPE_PATCH_FIELD_REQUIRED",
                                    "a patch input requires at least one optional field",
                                    source,
                                    patch.range,
                                );
                            }
                            for (field_name, patch_field) in &patch_fields {
                                if !patch_field.optional {
                                    self.push_diagnostic(
                                        "TYPE_PATCH_FIELD_NOT_OPTIONAL",
                                        format!(
                                            "patch input field `{}.{field_name}` must be declared `optional` so omission remains distinct from a supplied value",
                                            received.name
                                        ),
                                        source,
                                        patch.range,
                                    );
                                }
                                let Some(entity_field) = expected_fields
                                    .as_ref()
                                    .and_then(|fields| fields.get(field_name))
                                else {
                                    self.push_diagnostic(
                                        "TYPE_PATCH_UNKNOWN_FIELD",
                                        format!(
                                            "entity `{target}` has no field `{field_name}` supplied by patch input `{}`",
                                            received.name
                                        ),
                                        source,
                                        patch.range,
                                    );
                                    continue;
                                };
                                let expected = TypeValue {
                                    name: format!("{target}.{field_name}"),
                                    arguments: entity_field.declared_type.arguments.clone(),
                                    nullable: entity_field.declared_type.nullable,
                                };
                                self.require_compatible(
                                    &patch_field.declared_type,
                                    &expected,
                                    source,
                                    patch.range,
                                );
                            }
                            for conditional in &update.conditional_changes {
                                let supplied = &conditional.supplied;
                                let valid_binding = supplied.path.len() == 2
                                    && supplied.path[0].text == patch.path[0].text;
                                if !valid_binding {
                                    self.push_diagnostic(
                                        "TYPE_PATCH_CONDITION_BINDING",
                                        format!(
                                            "patch-derived condition must use `{}.<field> supplied`",
                                            patch.path[0].text
                                        ),
                                        source,
                                        supplied.range,
                                    );
                                    continue;
                                }
                                let supplied_field = &supplied.path[1].text;
                                if !patch_fields.contains_key(supplied_field) {
                                    self.push_diagnostic(
                                        "TYPE_PATCH_CONDITION_UNKNOWN_FIELD",
                                        format!(
                                            "patch input `{}` has no field `{supplied_field}`",
                                            received.name
                                        ),
                                        source,
                                        supplied.range,
                                    );
                                }
                            }
                            for patch_field in patch_fields.keys() {
                                if changed_fields.contains(patch_field) {
                                    self.push_diagnostic(
                                        "TYPE_PATCH_DERIVED_OVERLAP",
                                        format!(
                                            "field `{target}.{patch_field}` cannot be written by both `patch:` and derived `set:`"
                                        ),
                                        source,
                                        patch.range,
                                    );
                                }
                            }
                        }
                    }
                }
                if let Some(empty) = &update.empty {
                    self.check_failure_binding_fields(empty, environment, source);
                }
                self.check_failure_binding_fields(&update.missing, environment, source);
                self.check_conflict_bindings(&target, &update.conflicts, environment, source);
                Some(simple_type(&target))
            }
            Expression::Delete(delete) => {
                let target = joined_name(&delete.target.path);
                if !self.catalogue.entities.contains(&target) {
                    self.push_diagnostic(
                        "TYPE_DELETE_NOT_ENTITY",
                        format!("`delete` requires an entity, but `{target}` is not persistent"),
                        source,
                        delete.target.range,
                    );
                }
                self.check_mutation_predicate(
                    "delete",
                    &target,
                    &delete.field,
                    &delete.value,
                    environment,
                    source,
                );
                self.check_failure_binding_fields(&delete.missing, environment, source);
                self.check_conflict_bindings(&target, &delete.conflicts, environment, source);
                Some(simple_type(&target))
            }
            Expression::Unary(unary) => {
                let value = self.infer_expression(&unary.value, environment, source);
                if let Some(value) = value {
                    let root = self.representation_root(&value.name);
                    match unary.operator {
                        jadpo_syntax::UnaryOperator::Not => {
                            if value.nullable || root.as_deref() != Some("Bool") {
                                self.push_diagnostic(
                                    "TYPE_UNARY_OPERAND",
                                    format!(
                                        "`not` requires `Bool`, received `{}`",
                                        value.display()
                                    ),
                                    source,
                                    unary.range,
                                );
                            }
                            Some(simple_type("Bool"))
                        }
                        jadpo_syntax::UnaryOperator::Negate => {
                            if value.nullable || !matches!(root.as_deref(), Some("Int" | "Decimal"))
                            {
                                self.push_diagnostic(
                                    "TYPE_UNARY_OPERAND",
                                    format!(
                                        "numeric negation requires `Int` or `Decimal`, received `{}`",
                                        value.display()
                                    ),
                                    source,
                                    unary.range,
                                );
                            }
                            Some(root.map_or_else(|| simple_type("Int"), |root| simple_type(&root)))
                        }
                    }
                } else {
                    None
                }
            }
            Expression::Binary(binary) => {
                let left = self.infer_expression(&binary.left, environment, source);
                let right = self.infer_expression(&binary.right, environment, source);
                if let (Some(left), Some(right)) = (left, right) {
                    self.infer_binary(binary, &left, &right, source)
                } else {
                    None
                }
            }
            Expression::Grouped(grouped) => {
                self.infer_expression(&grouped.value, environment, source)
            }
            Expression::Missing(_) => None,
        };

        if let Some(inferred) = &inferred {
            self.result.expressions.push(InferredExpression {
                source: source.to_owned(),
                range: expression.range(),
                type_name: inferred.display(),
            });
        }
        inferred
    }

    fn infer_binary(
        &mut self,
        binary: &jadpo_syntax::BinaryExpression,
        left: &TypeValue,
        right: &TypeValue,
        source: &str,
    ) -> Option<TypeValue> {
        use jadpo_syntax::BinaryOperator;
        let comparable = self.representation_compatible(left, right)
            || self.representation_compatible(right, left);
        let left_root = self.representation_root(&left.name);
        let right_root = self.representation_root(&right.name);
        let non_nullable = !left.nullable && !right.nullable;
        match binary.operator {
            BinaryOperator::Equal | BinaryOperator::NotEqual => {
                if !comparable {
                    self.push_diagnostic(
                        "TYPE_INCOMPARABLE",
                        format!(
                            "values of type `{}` and `{}` cannot be compared",
                            left.display(),
                            right.display()
                        ),
                        source,
                        binary.range,
                    );
                }
                Some(simple_type("Bool"))
            }
            BinaryOperator::And | BinaryOperator::Or => {
                if !non_nullable
                    || left_root.as_deref() != Some("Bool")
                    || right_root.as_deref() != Some("Bool")
                {
                    self.push_diagnostic(
                        "TYPE_LOGICAL_OPERAND",
                        format!(
                            "logical operators require two `Bool` values, received `{}` and `{}`",
                            left.display(),
                            right.display()
                        ),
                        source,
                        binary.range,
                    );
                }
                Some(simple_type("Bool"))
            }
            BinaryOperator::Less
            | BinaryOperator::LessEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterEqual => {
                let ordered = matches!(
                    left_root.as_deref(),
                    Some("Int" | "Decimal" | "Text" | "DateTime")
                );
                if !non_nullable || !comparable || !ordered || left_root != right_root {
                    self.push_diagnostic(
                        "TYPE_ORDERING_OPERAND",
                        format!(
                            "ordering requires compatible non-null Text, numeric, or DateTime values; received `{}` and `{}`",
                            left.display(),
                            right.display()
                        ),
                        source,
                        binary.range,
                    );
                }
                Some(simple_type("Bool"))
            }
            BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Remainder => {
                let numeric = matches!(left_root.as_deref(), Some("Int" | "Decimal"))
                    && matches!(right_root.as_deref(), Some("Int" | "Decimal"));
                let text_add = binary.operator == BinaryOperator::Add
                    && left_root.as_deref() == Some("Text")
                    && right_root.as_deref() == Some("Text");
                if !non_nullable || (!numeric && !text_add) {
                    self.push_diagnostic(
                        "TYPE_ARITHMETIC_OPERAND",
                        format!(
                            "arithmetic requires numeric values (or two Text values for `+`); received `{}` and `{}`",
                            left.display(),
                            right.display()
                        ),
                        source,
                        binary.range,
                    );
                }
                if text_add {
                    Some(simple_type("Text"))
                } else if left_root.as_deref() == Some("Decimal")
                    || right_root.as_deref() == Some("Decimal")
                    || binary.operator == BinaryOperator::Divide
                {
                    Some(simple_type("Decimal"))
                } else {
                    Some(simple_type("Int"))
                }
            }
        }
    }

    fn check_mutation_predicate(
        &mut self,
        operation: &str,
        target: &str,
        field: &Name,
        value: &Expression,
        environment: &BTreeMap<String, TypeValue>,
        source: &str,
    ) {
        let expected = self
            .catalogue
            .records
            .get(target)
            .and_then(|fields| fields.get(&field.text))
            .cloned();
        let Some(expected) = expected else {
            if self.catalogue.records.contains_key(target) {
                self.push_diagnostic(
                    "TYPE_MUTATION_UNKNOWN_PREDICATE_FIELD",
                    format!("entity `{target}` has no field `{}`", field.text),
                    source,
                    field.range,
                );
            }
            self.infer_expression(value, environment, source);
            return;
        };
        if expected.declared_type.nullable {
            self.push_diagnostic(
                "TYPE_MUTATION_NULLABLE_FIELD_UNSUPPORTED",
                format!(
                    "{operation} predicates cannot yet target nullable field `{target}.{}`",
                    field.text
                ),
                source,
                field.range,
            );
        }
        if let Some(received) = self.infer_expression(value, environment, source) {
            let field_type = TypeValue {
                name: format!("{target}.{}", field.text),
                arguments: expected.declared_type.arguments,
                nullable: expected.declared_type.nullable,
            };
            self.require_compatible(&received, &field_type, source, value.range());
        }
    }

    fn check_failure_binding_fields(
        &mut self,
        binding: &jadpo_syntax::RejectStatement,
        environment: &BTreeMap<String, TypeValue>,
        source: &str,
    ) {
        self.check_reject_fields(
            &binding.failure.text,
            "public",
            &binding.public_values,
            environment,
            source,
        );
        self.check_reject_fields(
            &binding.failure.text,
            "internal",
            &binding.internal_values,
            environment,
            source,
        );
    }

    fn check_conflict_bindings(
        &mut self,
        target: &str,
        bindings: &[jadpo_syntax::ConflictBinding],
        environment: &BTreeMap<String, TypeValue>,
        source: &str,
    ) {
        let mut seen = BTreeSet::new();
        for binding in bindings {
            self.check_failure_binding_fields(&binding.rejection, environment, source);
            let (key, range) = binding.constraint.as_ref().map_or_else(
                || ("<fallback>".to_owned(), binding.range),
                |constraint| {
                    let name = joined_name(&constraint.path);
                    let name = if constraint.path.len() == 1 {
                        format!("{target}.{name}")
                    } else {
                        name
                    };
                    (name, constraint.range)
                },
            );
            if !seen.insert(key.clone()) {
                self.push_diagnostic(
                    "TYPE_CONFLICT_DUPLICATE_BINDING",
                    if key == "<fallback>" {
                        "a mutation may declare at most one fallback conflict binding".to_owned()
                    } else {
                        format!("constraint `{key}` is mapped more than once")
                    },
                    source,
                    range,
                );
            }
            if key != "<fallback>" && !self.catalogue.persistence_constraints.contains(&key) {
                self.push_diagnostic(
                    "TYPE_CONFLICT_UNKNOWN_CONSTRAINT",
                    format!("`{key}` is not a compiler-known persistence constraint"),
                    source,
                    range,
                );
            }
        }
    }

    fn infer_invocation(
        &mut self,
        invocation: &InvocationExpression,
        environment: &BTreeMap<String, TypeValue>,
        source: &str,
    ) -> Option<TypeValue> {
        let callee = joined_name(&invocation.callee.path);
        if let Some(signature) = self.catalogue.callables.get(&callee).cloned() {
            if signature.parameters.len() != invocation.arguments.len() {
                self.push_diagnostic(
                    "TYPE_ARGUMENT_COUNT",
                    format!(
                        "`{callee}` expects {} argument(s), but {} were supplied",
                        signature.parameters.len(),
                        invocation.arguments.len()
                    ),
                    source,
                    invocation.range,
                );
            }
            for (argument, expected) in invocation.arguments.iter().zip(&signature.parameters) {
                if let Some(received) = self.infer_expression(argument, environment, source) {
                    self.require_compatible(&received, expected, source, argument.range());
                }
            }
            return Some(signature.result);
        }

        let Some(target_node) = self.graph.node(&callee) else {
            for argument in &invocation.arguments {
                self.infer_expression(argument, environment, source);
            }
            return None;
        };
        if !target_node.kind.is_type() {
            return None;
        }
        if invocation.arguments.len() != 1 {
            self.push_diagnostic(
                "TYPE_CONSTRUCTOR_ARGUMENT_COUNT",
                format!("validated constructor `{callee}` requires exactly one argument"),
                source,
                invocation.range,
            );
            return Some(simple_type(&callee));
        }

        let argument = &invocation.arguments[0];
        let argument_type = self.infer_expression(argument, environment, source);
        if let Some(argument_type) = &argument_type {
            if !self.constructor_input_compatible(argument_type, &callee) {
                self.push_diagnostic(
                    "TYPE_CONSTRUCTOR_INPUT",
                    format!(
                        "cannot validate `{}` as `{callee}` because their representations differ",
                        argument_type.display()
                    ),
                    source,
                    argument.range(),
                );
            }
        }
        if let Expression::Literal(literal) = argument {
            if let Some(reason) = self.invalid_literal_reason(&callee, literal) {
                self.push_diagnostic(
                    "TYPE_INVALID_LITERAL",
                    format!("literal is not valid for `{callee}`: {reason}"),
                    source,
                    invocation.range,
                );
            }
        }
        Some(simple_type(&callee))
    }

    fn check_construction(
        &mut self,
        target: &str,
        fields: &[FieldInitialiser],
        range: TextRange,
        environment: &BTreeMap<String, TypeValue>,
        source: &str,
        use_field_identity: bool,
    ) {
        let Some(expected_fields) = self.catalogue.records.get(target).cloned() else {
            self.push_diagnostic(
                "TYPE_NOT_RECORD",
                format!("`{target}` is not a constructible record"),
                source,
                range,
            );
            return;
        };
        let supplied = fields
            .iter()
            .map(|field| field.name.text.as_str())
            .collect::<BTreeSet<_>>();

        for (name, expected) in &expected_fields {
            if !expected.optional && !supplied.contains(name.as_str()) {
                self.push_diagnostic(
                    "TYPE_MISSING_FIELD",
                    format!("construction of `{target}` is missing required field `{name}`"),
                    source,
                    range,
                );
            }
        }
        for field in fields {
            let Some(expected) = expected_fields.get(&field.name.text) else {
                self.push_diagnostic(
                    "TYPE_UNKNOWN_FIELD",
                    format!("`{}` has no field `{}`", target, field.name.text),
                    source,
                    field.name.range,
                );
                continue;
            };
            if let Some(received) = self.infer_expression(&field.value, environment, source) {
                let expected_type = if use_field_identity {
                    TypeValue {
                        name: format!("{target}.{}", field.name.text),
                        arguments: expected.declared_type.arguments.clone(),
                        nullable: expected.declared_type.nullable,
                    }
                } else {
                    expected.declared_type.clone()
                };
                self.require_compatible(&received, &expected_type, source, field.value.range());
            }
        }
    }

    fn check_variant_construction(
        &mut self,
        target: &str,
        expected_fields: &BTreeMap<String, RecordField>,
        supplied_fields: &[FieldInitialiser],
        range: TextRange,
        environment: &BTreeMap<String, TypeValue>,
        source: &str,
    ) {
        let supplied = supplied_fields
            .iter()
            .map(|field| field.name.text.as_str())
            .collect::<BTreeSet<_>>();
        for (name, expected) in expected_fields {
            if !expected.optional && !supplied.contains(name.as_str()) {
                self.push_diagnostic(
                    "TYPE_MISSING_VARIANT_FIELD",
                    format!("construction of `{target}` is missing required field `{name}`"),
                    source,
                    range,
                );
            }
        }
        let mut seen = BTreeSet::new();
        for field in supplied_fields {
            if !seen.insert(field.name.text.clone()) {
                self.push_diagnostic(
                    "TYPE_DUPLICATE_VARIANT_FIELD",
                    format!(
                        "variant field `{}` is supplied more than once",
                        field.name.text
                    ),
                    source,
                    field.name.range,
                );
                continue;
            }
            let Some(expected) = expected_fields.get(&field.name.text) else {
                self.push_diagnostic(
                    "TYPE_UNKNOWN_VARIANT_FIELD",
                    format!("`{target}` has no field `{}`", field.name.text),
                    source,
                    field.name.range,
                );
                continue;
            };
            if let Some(received) = self.infer_expression(&field.value, environment, source) {
                self.require_compatible(
                    &received,
                    &expected.declared_type,
                    source,
                    field.value.range(),
                );
            }
        }
    }

    fn enum_variant(
        &self,
        target: &str,
    ) -> Option<(String, String, BTreeMap<String, RecordField>)> {
        let (enum_name, variant) = target.split_once('.')?;
        if variant.contains('.') {
            return None;
        }
        let fields = self.catalogue.enums.get(enum_name)?.get(variant)?.clone();
        Some((enum_name.to_owned(), variant.to_owned(), fields))
    }

    fn check_reject_fields(
        &mut self,
        failure: &str,
        scope: &str,
        fields: &[FieldInitialiser],
        environment: &BTreeMap<String, TypeValue>,
        source: &str,
    ) {
        let expected_fields = self
            .catalogue
            .failure_fields
            .get(&format!("{failure}.{scope}"))
            .cloned()
            .unwrap_or_default();
        for field in fields {
            let inferred = self.infer_expression(&field.value, environment, source);
            if let (Some(received), Some(expected)) =
                (inferred, expected_fields.get(&field.name.text))
            {
                self.require_compatible(
                    &received,
                    &expected.declared_type,
                    source,
                    field.value.range(),
                );
            }
        }
    }

    fn resolve_value_path(
        &mut self,
        path: &[Name],
        environment: &BTreeMap<String, TypeValue>,
        source: &str,
    ) -> Option<TypeValue> {
        let first = path.first()?;
        if path.len() == 2 {
            if let Some(variants) = self.catalogue.enums.get(&first.text) {
                let variant = &path[1];
                if variants.contains_key(&variant.text) {
                    return Some(simple_type(&first.text));
                }
                self.push_diagnostic(
                    "TYPE_UNKNOWN_ENUM_VARIANT",
                    format!("enum `{}` has no variant `{}`", first.text, variant.text),
                    source,
                    variant.range,
                );
                return None;
            }
        }
        let Some(mut current) = environment.get(&first.text).cloned() else {
            self.push_diagnostic(
                "TYPE_UNKNOWN_VALUE",
                format!("unknown value `{}`", first.text),
                source,
                first.range,
            );
            return None;
        };

        for field in &path[1..] {
            if current.nullable {
                self.push_diagnostic(
                    "TYPE_NULLABLE_SELECTION",
                    format!(
                        "cannot select field `{}` from nullable type `{}` without handling `none`",
                        field.text,
                        current.display()
                    ),
                    source,
                    field.range,
                );
                return None;
            }
            let Some(record_name) = self.record_shape_name(&current.name) else {
                self.push_diagnostic(
                    "TYPE_FIELD_ON_NON_RECORD",
                    format!("type `{}` has no selectable fields", current.display()),
                    source,
                    field.range,
                );
                return None;
            };
            let declared = self
                .catalogue
                .records
                .get(&record_name)
                .and_then(|fields| fields.get(&field.text))
                .cloned();
            let Some(declared) = declared else {
                self.push_diagnostic(
                    "TYPE_UNKNOWN_FIELD",
                    format!("`{record_name}` has no field `{}`", field.text),
                    source,
                    field.range,
                );
                return None;
            };
            current = TypeValue {
                name: format!("{record_name}.{}", field.text),
                arguments: declared.declared_type.arguments,
                nullable: declared.declared_type.nullable,
            };
        }
        Some(current)
    }

    fn require_compatible(
        &mut self,
        received: &TypeValue,
        expected: &TypeValue,
        source: &str,
        range: TextRange,
    ) {
        if self.assignment_compatible(received, expected) {
            return;
        }
        let shared_parent = self.shared_parent(received, expected);
        let both_fields = self.node_kind(&received.name) == Some(NodeKind::Field)
            && self.node_kind(&expected.name) == Some(NodeKind::Field);
        if both_fields && shared_parent.is_some() {
            self.push_diagnostic(
                "TYPE_SIBLING_MISMATCH",
                format!(
                    "expected `{}`, received sibling `{}`; shared parent is `{}`",
                    expected.display(),
                    received.display(),
                    shared_parent.unwrap()
                ),
                source,
                range,
            );
        } else {
            self.push_diagnostic(
                "TYPE_MISMATCH",
                format!(
                    "expected `{}`, received `{}`",
                    expected.display(),
                    received.display()
                ),
                source,
                range,
            );
        }
    }

    fn assignment_compatible(&self, received: &TypeValue, expected: &TypeValue) -> bool {
        if received.name == "none" {
            return expected.nullable;
        }
        if received == expected {
            return true;
        }
        if received.nullable && !expected.nullable {
            return false;
        }
        if received.arguments != expected.arguments {
            return false;
        }
        if self.node_kind(&expected.name) == Some(NodeKind::PreludeType) {
            return false;
        }
        self.has_refinement_path(&received.name, &expected.name)
    }

    fn representation_compatible(&self, left: &TypeValue, right: &TypeValue) -> bool {
        left.arguments == right.arguments
            && (left.name == right.name || self.has_refinement_path(&left.name, &right.name))
    }

    fn constructor_input_compatible(&self, argument: &TypeValue, target: &str) -> bool {
        let argument_root = self.representation_root(&argument.name);
        let target_root = self.representation_root(target);
        argument_root.is_some() && argument_root == target_root
    }

    fn has_refinement_path(&self, from: &str, to: &str) -> bool {
        let mut current = self.graph.node(from).map(|node| node.id);
        let Some(target) = self.graph.node(to).map(|node| node.id) else {
            return false;
        };
        let mut visited = BTreeSet::new();
        while let Some(node) = current {
            if node == target {
                return true;
            }
            if !visited.insert(node) {
                break;
            }
            current = self
                .graph
                .refinements
                .iter()
                .find(|edge| edge.refined == node)
                .map(|edge| edge.parent);
        }
        false
    }

    fn ancestors(&self, name: &str) -> Vec<String> {
        let mut ancestors = Vec::new();
        let mut current = self.graph.node(name).map(|node| node.id);
        let mut visited = BTreeSet::new();
        while let Some(node) = current {
            if !visited.insert(node) {
                break;
            }
            ancestors.push(self.graph.nodes[node.0 as usize].name.clone());
            current = self
                .graph
                .refinements
                .iter()
                .find(|edge| edge.refined == node)
                .map(|edge| edge.parent);
        }
        ancestors
    }

    fn shared_parent(&self, left: &TypeValue, right: &TypeValue) -> Option<String> {
        let right_ancestors = self
            .ancestors(&right.name)
            .into_iter()
            .collect::<BTreeSet<_>>();
        self.ancestors(&left.name)
            .into_iter()
            .skip(1)
            .find(|ancestor| right_ancestors.contains(ancestor))
    }

    fn representation_root(&self, name: &str) -> Option<String> {
        self.ancestors(name)
            .into_iter()
            .rev()
            .find(|ancestor| self.node_kind(ancestor) == Some(NodeKind::PreludeType))
    }

    fn record_shape_name(&self, name: &str) -> Option<String> {
        self.ancestors(name)
            .into_iter()
            .find(|candidate| self.catalogue.records.contains_key(candidate))
    }

    fn enum_shape_name(&self, name: &str) -> Option<String> {
        self.ancestors(name)
            .into_iter()
            .find(|candidate| self.catalogue.enums.contains_key(candidate))
    }

    fn node_kind(&self, name: &str) -> Option<NodeKind> {
        self.graph.node(name).map(|node| node.kind)
    }

    fn is_primitive_signature_type(&self, value: &TypeValue) -> bool {
        value.arguments.is_empty()
            && self.node_kind(&value.name) == Some(NodeKind::PreludeType)
            && value.name != "Unit"
    }

    fn literal_type(&self, literal: &Literal) -> Option<TypeValue> {
        match literal.kind {
            LiteralKind::String => Some(simple_type("Text")),
            LiteralKind::Integer => Some(simple_type("Int")),
            LiteralKind::Decimal => Some(simple_type("Decimal")),
            LiteralKind::Boolean => Some(simple_type("Bool")),
            LiteralKind::None => Some(simple_type("none")),
        }
    }

    fn invalid_literal_reason(&self, target: &str, literal: &Literal) -> Option<String> {
        let constraints = self.constraints_for(target);
        let text = unquote(&literal.text);
        for constraint in constraints {
            let violated = match constraint.kind {
                ConstraintKind::MinLength => constraint
                    .value
                    .text
                    .parse::<usize>()
                    .ok()
                    .is_some_and(|minimum| text.chars().count() < minimum),
                ConstraintKind::MaxLength => constraint
                    .value
                    .text
                    .parse::<usize>()
                    .ok()
                    .is_some_and(|maximum| text.chars().count() > maximum),
                ConstraintKind::Min => numeric_literal(literal)
                    .zip(numeric_constraint(constraint))
                    .is_some_and(|(value, minimum)| value < minimum),
                ConstraintKind::Max => numeric_literal(literal)
                    .zip(numeric_constraint(constraint))
                    .is_some_and(|(value, maximum)| value > maximum),
                ConstraintKind::Format => constraint.value.text == "email" && !valid_email(&text),
                ConstraintKind::Pattern => {
                    !matches_core_pattern(&text, &unquote(&constraint.value.text))
                }
            };
            if violated {
                return Some(format!(
                    "constraint `{}` was not satisfied",
                    constraint_name(constraint.kind)
                ));
            }
        }
        None
    }

    fn constraints_for(&self, target: &str) -> Vec<&Constraint> {
        self.ancestors(target)
            .iter()
            .flat_map(|name| self.catalogue.constraints.get(name).into_iter().flatten())
            .collect()
    }

    fn push_diagnostic(
        &mut self,
        code: &'static str,
        message: impl Into<String>,
        source: &str,
        range: TextRange,
    ) {
        let mut diagnostic = Diagnostic::error(code, message);
        diagnostic.primary = Some(SourceSpan {
            source: source.to_owned(),
            start: range.start,
            end: range.end,
        });
        self.result.diagnostics.push(diagnostic);
    }

    fn check_query_pagination(
        &mut self,
        pagination: &jadpo_syntax::QueryPagination,
        environment: &BTreeMap<String, TypeValue>,
        source: &str,
    ) {
        for (label, expression, minimum) in [
            ("limit", pagination.limit.as_ref(), 1_i64),
            ("offset", pagination.offset.as_ref(), 0_i64),
        ] {
            if let Some(received) = self.infer_expression(expression, environment, source) {
                self.require_compatible(&received, &simple_type("Int"), source, expression.range());
            }
            let Expression::Literal(literal) = expression else {
                self.push_diagnostic(
                    "TYPE_QUERY_PAGINATION_CONSTANT_REQUIRED",
                    format!(
                        "query `{label}` must be an integer literal in the first pagination slice"
                    ),
                    source,
                    expression.range(),
                );
                continue;
            };
            let value = (literal.kind == LiteralKind::Integer)
                .then(|| literal.text.parse::<i64>().ok())
                .flatten();
            if !value.is_some_and(|value| value >= minimum) {
                self.push_diagnostic(
                    "TYPE_QUERY_PAGINATION_RANGE",
                    format!(
                        "query `{label}` must be an integer greater than or equal to {minimum}"
                    ),
                    source,
                    literal.range,
                );
            }
        }
    }
}

fn type_value(reference: &TypeReference) -> TypeValue {
    TypeValue {
        name: joined_name(&reference.path),
        arguments: reference.arguments.iter().map(type_value).collect(),
        nullable: reference.nullable,
    }
}

fn simple_type(name: &str) -> TypeValue {
    TypeValue {
        name: name.to_owned(),
        arguments: Vec::new(),
        nullable: false,
    }
}

fn joined_name(path: &[Name]) -> String {
    path.iter()
        .map(|name| name.text.as_str())
        .collect::<Vec<_>>()
        .join(".")
}

fn unquote(value: &str) -> String {
    let value = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value);
    let mut result = String::new();
    let mut chars = value.chars();
    while let Some(character) = chars.next() {
        if character != '\\' {
            result.push(character);
            continue;
        }
        match chars.next() {
            Some('n') => result.push('\n'),
            Some('r') => result.push('\r'),
            Some('t') => result.push('\t'),
            Some('"') => result.push('"'),
            Some('\\') => result.push('\\'),
            Some(other) => result.push(other),
            None => result.push('\\'),
        }
    }
    result
}

fn numeric_literal(literal: &Literal) -> Option<f64> {
    literal.text.parse().ok()
}

fn numeric_constraint(constraint: &Constraint) -> Option<f64> {
    constraint.value.text.parse().ok()
}

fn valid_email(value: &str) -> bool {
    let mut pieces = value.split('@');
    let local = pieces.next().unwrap_or_default();
    let domain = pieces.next().unwrap_or_default();
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && pieces.next().is_none()
        && !value.chars().any(char::is_whitespace)
}

fn matches_core_pattern(value: &str, pattern: &str) -> bool {
    match pattern {
        "[a-z0-9_]+" => {
            !value.is_empty()
                && value.chars().all(|character| {
                    character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
                })
        }
        _ => true,
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

#[cfg(test)]
mod tests {
    use super::check_types;
    use crate::build_semantic_graph;
    use jadpo_syntax::parse;
    use std::path::Path;

    fn check(source: &str) -> super::TypeCheckResult {
        let parsed = parse(Path::new("test.jadpo"), source);
        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let graph = build_semantic_graph(std::slice::from_ref(&parsed));
        assert!(graph.diagnostics.is_empty(), "{:#?}", graph.diagnostics);
        check_types(&[parsed], &graph)
    }

    #[test]
    fn widens_fields_but_rejects_siblings() {
        let result = check(
            r#"
type Email = Text { format email }
entity Customer { email: Email }
entity Supplier { email: Email }
function receipt(email: Customer.email) -> Customer.email { return email }
function misuse(supplier: Supplier) -> Customer.email { return receipt(supplier.email) }
"#,
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "TYPE_SIBLING_MISMATCH");
    }

    #[test]
    fn validates_constant_constructors() {
        let result = check(
            r#"
type Email = Text { format email }
function email() -> Email { return Email("not-an-email") }
"#,
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "TYPE_INVALID_LITERAL");
    }

    #[test]
    fn types_an_included_inverse_as_its_named_output_shape() {
        let result = check(
            r#"
entity User { id: Uuid identity inverse todos: many Todo via Todo.owner_id }
entity Todo { id: Uuid identity owner_id: User.id references User.id on_delete cascade }
output UserTodos { parent: User todos: List<Todo> }
failure UserNotFound: NotFound { code "user_not_found" }
action load(user_id: User.id) -> UserTodos fails UserNotFound {
    return query required User {
        where: id == user_id
        include: todos into: UserTodos order_by: id asc limit: 100 offset: 0
        missing: UserNotFound
    }
}
"#,
        );
        assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
        assert!(result
            .expressions
            .iter()
            .any(|expression| expression.type_name == "UserTodos"));
    }

    #[test]
    fn rejects_unbounded_or_negative_relationship_pagination() {
        let result = check(
            r#"
entity User { id: Uuid identity inverse todos: many Todo via Todo.owner_id }
entity Todo { id: Uuid identity owner_id: User.id references User.id on_delete cascade }
output UserTodos { parent: User todos: List<Todo> }
failure UserNotFound: NotFound { code "user_not_found" }
action load(user_id: User.id) -> UserTodos fails UserNotFound {
    return query required User {
        where: id == user_id
        include: todos into: UserTodos order_by: id asc limit: 0 offset: -1
        missing: UserNotFound
    }
}
"#,
        );
        assert_eq!(
            result
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == "TYPE_QUERY_PAGINATION_RANGE")
                .count(),
            2
        );
    }

    #[test]
    fn types_a_paginated_many_parent_include_as_a_list_of_output_shapes() {
        let result = check(
            r#"
entity User {
    id: Uuid identity
    group: Text index
    inverse todos: many Todo via Todo.owner_id
}
entity Todo { id: Uuid identity owner_id: User.id references User.id on_delete cascade }
output UserTodos { parent: User todos: List<Todo> }
action load(group: User.group) -> List<UserTodos> {
    return query many User {
        where: group == group
        order_by: id asc
        limit: 20 offset: 0
        include: todos into: UserTodos order_by: id asc limit: 10 offset: 0
    }
}
"#,
        );
        assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
        assert!(result
            .expressions
            .iter()
            .any(|expression| expression.type_name == "List<UserTodos>"));
    }

    #[test]
    fn permits_only_explicit_mutable_local_reassignment() {
        let result = check(
            r#"
type Choice = Text {}
function choose(initial: Choice, replacement: Choice) -> Choice {
    var mut selected: Choice = initial
    selected = replacement
    return selected
}
"#,
        );
        assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    }

    #[test]
    fn rejects_parameter_and_immutable_local_reassignment() {
        let result = check(
            r#"
type Choice = Text {}
type OtherChoice = Text {}
function replace_parameter(original: Choice, replacement: Choice) -> Choice {
    original = replacement
    return original
}
function replace_fixed(initial: Choice, replacement: Choice) -> Choice {
    var fixed: Choice = initial
    fixed = replacement
    return fixed
}
function replace_with_wrong_type(initial: Choice, replacement: OtherChoice) -> Choice {
    var mut selected: Choice = initial
    selected = (replacement)
    return selected
}
"#,
        );
        assert_eq!(
            result
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code)
                .collect::<Vec<_>>(),
            vec![
                "TYPE_ASSIGN_IMMUTABLE",
                "TYPE_ASSIGN_IMMUTABLE",
                "TYPE_MISMATCH"
            ]
        );
    }

    #[test]
    fn types_multiple_inverses_as_one_exact_output_shape() {
        let result = check(
            r#"
entity User {
    id: Uuid identity
    inverse todos: many Todo via Todo.owner_id
    inverse notes: many Note via Note.owner_id
}
entity Todo { id: Uuid identity owner_id: User.id references User.id on_delete cascade }
entity Note { id: Uuid identity owner_id: User.id references User.id on_delete cascade }
output UserActivity { parent: User todos: List<Todo> notes: List<Note> }
failure UserNotFound: NotFound { code "user_not_found" }
action load(user_id: User.id) -> UserActivity fails UserNotFound {
    return query required User {
        where: id == user_id
        include: todos into: UserActivity order_by: id asc limit: 10 offset: 0
        include: notes into: UserActivity order_by: id desc limit: 5 offset: 0
        missing: UserNotFound
    }
}
"#,
        );
        assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
        assert!(result
            .expressions
            .iter()
            .any(|expression| expression.type_name == "UserActivity"));
    }
}
