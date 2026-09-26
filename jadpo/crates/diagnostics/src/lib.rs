use std::fmt;

include!(concat!(env!("OUT_DIR"), "/catalogue_codes.rs"));

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

impl fmt::Display for Severity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => formatter.write_str("error"),
            Self::Warning => formatter.write_str("warning"),
            Self::Note => formatter.write_str("note"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceSpan {
    pub source: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepairKind {
    AutomaticFix,
    GuidedChoice,
    HumanDecision,
}

impl RepairKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AutomaticFix => "automatic_fix",
            Self::GuidedChoice => "guided_choice",
            Self::HumanDecision => "human_decision",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecisionOwner {
    Compiler,
    Agent,
    Human,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiagnosticFact {
    Callable(String),
    DeclaredProblems(String),
    ReachableProblems(String),
    EventRevision(String),
    LocalRevision(String),
    Route(String),
    FoundValue(String),
    Expected(String),
}

impl DiagnosticFact {
    fn into_pair(self) -> (&'static str, String) {
        match self {
            Self::Callable(value) => ("callable", value),
            Self::DeclaredProblems(value) => ("declared", value),
            Self::ReachableProblems(value) => ("reachable", value),
            Self::EventRevision(value) => ("eventRevision", value),
            Self::LocalRevision(value) => ("localRevision", value),
            Self::Route(value) => ("route", value),
            Self::FoundValue(value) => ("found", value),
            Self::Expected(value) => ("expected", value),
        }
    }
}

impl DecisionOwner {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Compiler => "compiler",
            Self::Agent => "agent",
            Self::Human => "human",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextEdit {
    pub source: String,
    pub start: usize,
    pub end: usize,
    pub replacement: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepairStep {
    pub kind: RepairKind,
    pub title: String,
    pub reason: String,
    pub decision_owner: DecisionOwner,
    pub preferred: bool,
    pub edits: Vec<TextEdit>,
    pub behavioral_effect: String,
    pub public_contract_effect: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticImpact {
    pub behavioral: String,
    pub public_contract: String,
    pub affected: Vec<String>,
    pub query_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogueDefinition {
    pub rule_id: String,
    pub category: String,
    pub summary: String,
    pub reason: String,
    pub recommended_title: String,
    pub repair_kind: RepairKind,
    pub decision_owner: DecisionOwner,
    pub help_id: String,
    pub context_keys: Vec<&'static str>,
    pub fixtures: Vec<&'static str>,
    /// True only when the public copy is deliberately authored for this rule.
    /// Structural fallbacks remain visible as catalogue debt and cannot satisfy
    /// the strict diagnostic-copy conformance gate.
    pub authored_copy: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerDiagnostic {
    pub code: &'static str,
    pub rule_id: String,
    pub severity: Severity,
    pub message: String,
    pub reason: String,
    pub recommended_next_step: Box<RepairStep>,
    pub alternatives: Vec<RepairStep>,
    pub decision_owner: DecisionOwner,
    pub help_id: String,
    pub source_revision: String,
    pub context: Vec<(String, String)>,
    pub impact: Box<DiagnosticImpact>,
    pub primary: Option<SourceSpan>,
    pub related: Vec<SourceSpan>,
    pub notes: Vec<String>,
}

/// Compatibility name used by the existing compiler crates.
pub type Diagnostic = CompilerDiagnostic;

impl CompilerDiagnostic {
    pub fn error(code: &'static str) -> Self {
        Self::new(code, Severity::Error)
    }

    fn new(code: &'static str, severity: Severity) -> Self {
        let definition = catalogue_definition(code);
        let recommended_next_step = RepairStep {
            kind: definition.repair_kind,
            title: definition.recommended_title.clone(),
            reason: definition.reason.clone(),
            decision_owner: definition.decision_owner,
            preferred: definition.repair_kind == RepairKind::AutomaticFix,
            edits: Vec::new(),
            behavioral_effect:
                "No behavioural change is applied until the proposed edit is validated.".to_owned(),
            public_contract_effect: "Review generated contract impact before applying the edit."
                .to_owned(),
        };
        let alternatives = default_alternatives(code, &definition);
        let mut diagnostic = Self {
            code,
            rule_id: definition.rule_id,
            severity,
            message: definition.summary.clone(),
            reason: definition.reason,
            recommended_next_step: Box::new(recommended_next_step),
            alternatives,
            decision_owner: definition.decision_owner,
            help_id: definition.help_id,
            source_revision: "unversioned".to_owned(),
            context: Vec::new(),
            impact: Box::new(DiagnosticImpact {
                behavioral: "Compilation is blocked until this rule is satisfied.".to_owned(),
                public_contract: "No public-contract change has been applied.".to_owned(),
                affected: Vec::new(),
                query_id: None,
            }),
            primary: None,
            related: Vec::new(),
            notes: Vec::new(),
        };
        if code == "ROUTE_AUTH_VALUE_INVALID" {
            diagnostic.impact.behavioral =
                "Compilation is blocked until the route authentication boundary is chosen."
                    .to_owned();
            diagnostic.impact.public_contract =
                "The public route security boundary is unresolved; the compiler will not guess."
                    .to_owned();
            diagnostic.recommended_next_step.behavioral_effect =
                "No authentication behavior changes until a human chooses a valid boundary."
                    .to_owned();
            diagnostic.recommended_next_step.public_contract_effect =
                "The route's public security contract remains unchanged while compilation is blocked."
                    .to_owned();
        }
        diagnostic
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn warning(code: &'static str) -> Self {
        Self::new(code, Severity::Warning)
    }

    pub fn with_source_revision(mut self, revision: impl Into<String>) -> Self {
        self.source_revision = revision.into();
        self
    }

    pub fn with_fact(mut self, fact: DiagnosticFact) -> Self {
        let (key, value) = fact.into_pair();
        let definition = catalogue_definition(self.code);
        if definition.context_keys.contains(&key) {
            if let Some((_, current)) = self.context.iter_mut().find(|(current, _)| current == key)
            {
                *current = value;
            } else if self.context.len() < 16 {
                self.context.push((key.to_owned(), value));
            }
            self.refresh_catalogue_copy();
        }
        self
    }

    fn refresh_catalogue_copy(&mut self) {
        let definition = catalogue_definition(self.code);
        self.message = render_catalogue_text(&definition.summary, &self.context);
        self.reason = render_catalogue_text(&definition.reason, &self.context);
        self.recommended_next_step.title =
            render_catalogue_text(&definition.recommended_title, &self.context);
        self.recommended_next_step.reason = self.reason.clone();
    }

    pub fn with_impact(mut self, affected: impl Into<String>) -> Self {
        if self.impact.affected.len() < 16 {
            self.impact.affected.push(affected.into());
        }
        self
    }

    pub fn with_related(mut self, span: SourceSpan) -> Self {
        if self.related.len() < 16 {
            self.related.push(span);
        }
        self
    }

    pub fn with_edit(mut self, edit: TextEdit) -> Self {
        self.recommended_next_step.edits.push(edit);
        self
    }

    pub fn to_json(&self) -> String {
        let related = self
            .related
            .iter()
            .map(|span| {
                format!(
                    "{{\"source\":{},\"range\":{{\"start\":{},\"end\":{}}}}}",
                    json_string(&span.source),
                    span.start,
                    span.end
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let location = self.primary.as_ref().map_or_else(
            || "null".to_owned(),
            |primary| {
                format!(
                    "{{\"source\":{},\"range\":{{\"start\":{},\"end\":{}}},\"related\":[{related}]}}",
                    json_string(&primary.source),
                    primary.start,
                    primary.end
                )
            },
        );
        let diagnostic_id = self.primary.as_ref().map_or_else(
            || format!("{}@global", self.rule_id),
            |primary| {
                format!(
                    "{}@{}:{}:{}",
                    self.rule_id, primary.source, primary.start, primary.end
                )
            },
        );
        let context = self
            .context
            .iter()
            .map(|(key, value)| format!("{}:{}", json_string(key), json_string(value)))
            .collect::<Vec<_>>()
            .join(",");
        let affected = self
            .impact
            .affected
            .iter()
            .map(|value| json_string(value))
            .collect::<Vec<_>>()
            .join(",");
        let alternatives = self
            .alternatives
            .iter()
            .map(repair_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"schemaVersion\":2,\"diagnosticId\":{},\"sourceRevision\":{},\"summary\":{},\"reason\":{},\"recommendedNextStep\":{},\"alternatives\":[{alternatives}],\"ruleId\":{},\"severity\":{},\"location\":{location},\"context\":{{{context}}},\"impact\":{{\"behavioral\":{},\"publicContract\":{},\"affected\":[{affected}],\"queryId\":{}}},\"helpId\":{},\"decisionOwner\":{},\"legacyAliases\":[{}]}}",
            json_string(&diagnostic_id),
            json_string(&self.source_revision),
            json_string(&self.message),
            json_string(&self.reason),
            repair_json(&self.recommended_next_step),
            json_string(&self.rule_id),
            json_string(&self.severity.to_string()),
            json_string(&self.impact.behavioral),
            json_string(&self.impact.public_contract),
            self.impact.query_id.as_deref().map(json_string).unwrap_or_else(|| "null".to_owned()),
            json_string(&self.help_id),
            json_string(self.decision_owner.as_str()),
            json_string(self.code),
        )
    }
}

fn repair_json(repair: &RepairStep) -> String {
    let edits = repair
        .edits
        .iter()
        .map(|edit| {
            format!(
                "{{\"source\":{},\"range\":{{\"start\":{},\"end\":{}}},\"replacement\":{}}}",
                json_string(&edit.source),
                edit.start,
                edit.end,
                json_string(&edit.replacement)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"kind\":{},\"title\":{},\"reason\":{},\"decisionOwner\":{},\"preferred\":{},\"edits\":[{edits}],\"preview\":{{\"behavioral\":{},\"publicContract\":{}}}}}",
        json_string(repair.kind.as_str()),
        json_string(&repair.title),
        json_string(&repair.reason),
        json_string(repair.decision_owner.as_str()),
        repair.preferred,
        json_string(&repair.behavioral_effect),
        json_string(&repair.public_contract_effect),
    )
}

#[derive(Clone, Copy)]
struct AuthoredCopy {
    summary: &'static str,
    reason: &'static str,
    next: &'static str,
}

fn syntax_catalogue_copy(code: &str) -> Option<AuthoredCopy> {
    let (summary, reason, next) = match code {
        "SYN_CONSTRAINT_NON_ENTITY" => (
            "Persistence constraints are only valid on entities",
            "`constraint` describes a storage identity or index and therefore needs an `entity` declaration; value, input, and output records have no persistence boundary.",
            "Move the constraint to its owning entity, or remove it from this record",
        ),
        "SYN_DUPLICATE_FIELD_MODIFIER" => (
            "Field modifier is repeated",
            "Each field modifier may be written once. Repeating a modifier makes the field's authored storage or validation contract ambiguous.",
            "Keep one occurrence of the repeated field modifier",
        ),
        "SYN_EMPTY_IMPORT" => (
            "Import selects no declarations",
            "A selective import must name at least one declaration from the target module; an empty selection introduces no usable name.",
            "Add at least one imported declaration name, or remove the import",
        ),
        "SYN_ENUM_EMPTY" => (
            "Enum declares no variants",
            "An enum is a closed set of named values and must contain at least one variant so construction and matching have a defined domain.",
            "Add at least one enum variant",
        ),
        "SYN_EXPECTED_CONSTRAINT" => (
            "Expected a type constraint",
            "A type constraint block accepts only supported constraint names such as length, numeric bound, pattern, or format constraints.",
            "Use a supported constraint name or remove the invalid item",
        ),
        "SYN_EXPECTED_CONSTRAINT_VALUE" => (
            "Constraint requires a value",
            "The preceding constraint name is incomplete without the literal or name that defines its bound, pattern, or format.",
            "Add a value of the kind required by this constraint",
        ),
        "SYN_EXPECTED_DECLARATION" => (
            "`{found}` cannot start a top-level declaration",
            "Jadpo files accept `type`, `enum`, `entity`, `value`, `input`, `output`, `failure`, `function`, `action`, `test`, and `route` declarations at the top level. Route items such as `path:` belong inside a `route` block, so `{found}` cannot be parsed here.",
            "Move `{found}` into its owning declaration, or replace it with a top-level declaration",
        ),
        "SYN_EXPECTED_DELETE_ACTION" => (
            "Delete requires an entity target",
            "A `delete required` expression must name the entity whose stored record will be removed before its predicate and failure bindings can be parsed.",
            "Add the entity name after `delete required`",
        ),
        "SYN_EXPECTED_EXPRESSION" => (
            "Expected an expression",
            "This position requires a value-producing expression, such as a literal, name, invocation, constructor, match, or persistence expression.",
            "Add the value or operation intended at this location",
        ),
        "SYN_EXPECTED_FAILURE_ITEM" => (
            "Failure declaration contains an unsupported item",
            "A failure body accepts `kind`, `code`, `message`, `public`, and `internal` items only.",
            "Replace this item with a supported failure member or remove it",
        ),
        "SYN_EXPECTED_FIELD" => (
            "Expected a field declaration",
            "Record and field blocks contain `name: Type` declarations. The current source cannot begin a field and is not a valid block terminator.",
            "Write a `name: Type` field or remove the stray source",
        ),
        "SYN_EXPECTED_FIELD_INITIALISER" => (
            "Expected a field value",
            "A constructor or object body assigns fields with `name: expression`; each item needs a field name before its value.",
            "Add a named field initialiser or remove the stray item",
        ),
        "SYN_EXPECTED_HTTP_METHOD" => (
            "Route requires an HTTP method",
            "A route declaration starts with exactly one of `GET`, `POST`, `PUT`, `PATCH`, or `DELETE` before its path.",
            "Add a supported HTTP method before the route path",
        ),
        "SYN_EXPECTED_INVERSE_CARDINALITY" => (
            "Inverse relationship requires a cardinality",
            "An inverse relationship must declare whether it resolves one optional record or many records so generated query and output shapes are deterministic.",
            "Add the supported inverse cardinality intended for this relationship",
        ),
        "SYN_EXPECTED_INVOCATION" => (
            "Route `run:` requires an action invocation",
            "Named route behavior calls an action with parentheses. A bare name or another expression does not define the route's argument mapping.",
            "Call the action using `name(arguments)` after `run:`",
        ),
        "SYN_EXPECTED_LITERAL" => (
            "Expected a literal value",
            "This grammar position accepts a literal of the required kind rather than a name, invocation, or compound expression.",
            "Replace this source with the required literal value",
        ),
        "SYN_EXPECTED_NAME" => (
            "Expected a name",
            "This position identifies a declaration, field, binding, or qualified path and therefore requires a valid Jadpo name.",
            "Add a valid name beginning with a letter or underscore",
        ),
        "SYN_EXPECTED_ORDER_DIRECTION" => (
            "Query ordering requires `asc` or `desc`",
            "Every `order_by` field needs an explicit direction so result ordering is deterministic across storage adapters.",
            "Add `asc` or `desc` after the ordering field",
        ),
        "SYN_EXPECTED_PATCH_INPUT" => (
            "Patch update requires an input value",
            "The `patch:` item must name the input record whose present fields will be applied to the entity.",
            "Add the patch input name after `patch:`",
        ),
        "SYN_EXPECTED_ROUTE_ITEM" => (
            "Unsupported item inside route",
            "A route body accepts `auth:`, `path:`, `input:`, `output:`, `run:`, and `action:` items only.",
            "Use a supported route item or move this source outside the route",
        ),
        "SYN_EXPECTED_STATEMENT" => (
            "Expected a statement",
            "Callable and test blocks contain supported statements such as variable declarations, return, reject, conditionals, matches, assertions, and persistence operations.",
            "Add a supported statement or remove the stray source",
        ),
        "SYN_EXPECTED_SUPPLIED" => (
            "Patch condition requires `supplied`",
            "A conditional patch assignment must explicitly test whether an optional input field was supplied before using its value.",
            "Add `supplied` with the optional patch field",
        ),
        "SYN_EXPECTED_SUPPLIED_FIELD" => (
            "`supplied` requires an input field",
            "The `supplied` condition must name the optional input field whose presence controls the conditional patch assignment.",
            "Add the optional input field after `supplied`",
        ),
        "SYN_EXPECTED_UPDATE_BODY" => (
            "Update requires `set:` or `patch:`",
            "After its target and predicate, an update must choose either fixed authored changes with `set:` or omission-aware input changes with `patch:`.",
            "Add the intended `set:` or `patch:` update body",
        ),
        "SYN_FAILURE_CODE_REQUIRED" => (
            "Failure requires a stable public code",
            "Every failure declaration needs one string `code` so generated transports can identify the failure without exposing internal details.",
            "Add a unique string `code` to the failure declaration",
        ),
        "SYN_FAILURE_KIND_REQUIRED" => (
            "Failure requires a standard kind",
            "Every failure declaration must select a standard kind so route status mapping and failure handling are deterministic.",
            "Add a supported `kind` to the failure declaration",
        ),
        "SYN_IMPORT_REQUIRES_MODULE" => (
            "Imports require a module declaration",
            "A file that imports names must declare its own logical module first; otherwise visibility and dependency identity are undefined.",
            "Add a `module` header before the imports",
        ),
        "SYN_INVERSE_NON_ENTITY" => (
            "Inverse relationships are only valid on entities",
            "An inverse relationship is derived from stored owning references and cannot be declared on value, input, or output records.",
            "Move the inverse relationship to its entity, or remove it",
        ),
        "SYN_MATCH_PATTERN" => (
            "Invalid match pattern",
            "A match arm must use a pattern supported by the matched value, such as a variant, Boolean, nullable pattern, or wildcard.",
            "Replace this source with a supported match pattern",
        ),
        "SYN_MUTATION_CONFLICT_REQUIRED" => (
            "Mutation requires a conflict failure binding",
            "Required create, update, and delete operations must map storage conflicts into an authored domain failure rather than exposing adapter errors.",
            "Add at least one `conflict:` failure binding",
        ),
        "SYN_ROUTE_EXPORT_INVALID" => (
            "Routes cannot be exported with `public`",
            "Routes already define an external transport boundary. Module `public` exports apply to named declarations and cannot export an unnamed route.",
            "Remove `public` from the route declaration",
        ),
        "SYN_TYPE_PARENT_REQUIRED" => (
            "Refined type requires a parent type",
            "A named type declaration refines an existing semantic type and must name that parent before its optional constraint block.",
            "Add the parent type after `=`",
        ),
        "SYN_UNEXPECTED_TOKEN" => (
            "Expected {expected}",
            "Found `{found}` while parsing this construct. Jadpo requires {expected} at this location, so parsing stops rather than guessing the authored structure.",
            "Provide {expected}",
        ),
        "SYN_UNSUPPORTED_THROW" => (
            "Arbitrary `throw` is not supported",
            "Jadpo failures are declared, typed, and propagated through `reject` and `attempt`; arbitrary exceptions would escape that checked boundary.",
            "Replace `throw` with a declared failure and `reject`",
        ),
        "SYN_UNTERMINATED_STRING" => (
            "String literal is not closed",
            "The lexer reached the line or file boundary before finding the quote that ends this string literal.",
            "Add the closing quote, or remove the opening quote",
        ),
        _ => return None,
    };
    Some(AuthoredCopy {
        summary,
        reason,
        next,
    })
}

fn semantic_catalogue_copy(code: &str) -> Option<AuthoredCopy> {
    let (summary, reason, next) = match code {
        "SEM_DUPLICATE_DECLARATION" => (
            "Declaration name is already in use",
            "Two declarations in the same visible namespace cannot share a name because every reference must resolve to exactly one semantic definition.",
            "Rename one declaration or remove the duplicate",
        ),
        "SEM_UNKNOWN_NAME" => (
            "Name does not resolve in this scope",
            "The name is not a local binding, visible declaration, selected import, or supported built-in at this source location.",
            "Correct the name, declare it, or import it from its module",
        ),
        "SEM_WRONG_NAME_KIND" => (
            "Name resolves to the wrong kind of declaration",
            "This grammar position requires a specific semantic declaration kind, but the resolved name identifies a different kind and cannot be substituted safely.",
            "Use a declaration of the required kind at this location",
        ),
        "SEM_UNKNOWN_CALLEE" => (
            "Called function or action cannot be found",
            "An invocation must resolve to one visible `function` or `action`; no callable with this name is available in the current module and imports.",
            "Correct, declare, or import the function or action being called",
        ),
        "SEM_NOT_CALLABLE" => (
            "Resolved name cannot be called",
            "The name exists, but it identifies a type, record, field, value, or other non-callable declaration rather than a `function` or `action`.",
            "Use a function or action name, or remove the invocation parentheses",
        ),
        _ => return None,
    };
    Some(AuthoredCopy {
        summary,
        reason,
        next,
    })
}

fn failure_catalogue_copy(code: &str) -> Option<AuthoredCopy> {
    let (summary, reason, next) = match code {
        "EFFECT_FUNCTION_CALLS_ACTION" => (
            "Function cannot call an action",
            "Functions are deterministic and effect-free. Calling an action would allow persistence or other effects to escape through a function boundary.",
            "Move the call into an action, or change the callee to a pure function",
        ),
        "EFFECT_FUNCTION_PERSISTENCE" => (
            "Function cannot perform persistence",
            "Create, query, update, and delete expressions access stored state and are permitted only inside actions; functions remain pure and deterministic.",
            "Move the persistence expression into an action",
        ),
        "FAIL_ATTEMPT_REQUIRED" => (
            "Fallible expression requires `attempt`",
            "Every expression with a recoverable problem set must make propagation visible with `attempt`.",
            "Prefix the fallible expression with `attempt`",
        ),
        "FAIL_CONTEXT_FIELD_OVERLAP" => (
            "Failure context field has conflicting disclosure",
            "A flat failure value cannot be assigned safely when the declaration gives the same field both public and internal disclosure.",
            "Choose one disclosure class for the failure context field",
        ),
        "FAIL_DUPLICATE_CODE" => (
            "Failure code is already in use",
            "Public failure codes identify domain failures across generated transports and must be unique even when declaration names differ.",
            "Give one failure a distinct stable public code",
        ),
        "FAIL_DUPLICATE_CONTEXT_FIELD" => (
            "Failure context field is declared more than once",
            "A failure has one flat context object; repeating a field would make construction, disclosure, and serialization ambiguous.",
            "Keep one declaration of the context field",
        ),
        "FAIL_DUPLICATE_DECLARATION" => (
            "Problem is repeated in the `fails` set",
            "A callable's authored `fails` list is a mathematical set. Repeating a problem does not add behavior and obscures exact failure review.",
            "Remove the repeated problem from the `fails` list",
        ),
        "FAIL_MISSING_CONTEXT_FIELD" => (
            "Rejected failure is missing required context",
            "Every field declared by a failure must be supplied when that failure is rejected so its public and internal values are complete and typed.",
            "Add the missing field to the `reject` value",
        ),
        "FAIL_MUTATION_CONFLICT_NOT_CONFLICT" => (
            "Mutation conflict binding uses the wrong failure kind",
            "A storage uniqueness or write conflict must map to a declared failure whose standard kind is `Conflict`; another kind would produce the wrong transport behavior.",
            "Bind the conflict to a failure declared with `kind Conflict`",
        ),
        "FAIL_PATCH_EMPTY_NOT_INVALID_VALUE" => (
            "Empty patch binding uses the wrong failure kind",
            "A patch with no supplied changes is invalid input and must map to a failure whose standard kind is `InvalidValue`.",
            "Bind `empty:` to a failure declared with `kind InvalidValue`",
        ),
        "FAIL_REQUIRED_MUTATION_NOT_NOT_FOUND" => (
            "Required mutation missing binding uses the wrong failure kind",
            "When a required update or delete finds no entity, its `missing:` binding must identify a failure with standard kind `NotFound`.",
            "Bind `missing:` to a failure declared with `kind NotFound`",
        ),
        "FAIL_REQUIRED_QUERY_NOT_NOT_FOUND" => (
            "Required query missing binding uses the wrong failure kind",
            "When a required query finds no entity, its `missing:` binding must identify a failure with standard kind `NotFound`.",
            "Bind `missing:` to a failure declared with `kind NotFound`",
        ),
        "FAIL_STALE_DECLARATION" => (
            "Declared problem is not reachable",
            "A callable's authored `fails` set must exactly equal its reachable unhandled problem set.",
            "Remove the stale `fails` entry",
        ),
        "FAIL_UNDECLARED_PROPAGATION" => (
            "Reachable problem is missing from the `fails` set",
            "A callable must declare every problem that can escape its body after local handling; otherwise callers cannot reason about the complete failure surface.",
            "Add the reachable problem to `fails`, or handle it before it escapes",
        ),
        "FAIL_UNKNOWN_CONTEXT_FIELD" => (
            "Rejected failure contains an unknown context field",
            "A `reject` value may assign only fields declared by that failure; extra fields have no disclosure classification or generated schema.",
            "Remove the extra field or declare it on the failure",
        ),
        _ => return None,
    };
    Some(AuthoredCopy {
        summary,
        reason,
        next,
    })
}

fn route_catalogue_copy(code: &str) -> Option<AuthoredCopy> {
    let (summary, reason, next) = match code {
        "ROUTE_AUTH_VALUE_INVALID" => (
            "Route authentication value `{found}` is not valid",
            "Routes require authentication by default. The only explicit opt-out is the exact form `auth: none`, so the compiler cannot infer whether `{found}` was meant to retain authentication or disable it.",
            "Choose the authentication boundary for `{route}`",
        ),
        "ROUTE_BEHAVIOUR_CONFLICT" => (
            "Route declares two behavior forms",
            "A route must select exactly one local inline `action:` or one named `run:` invocation; keeping both would create two competing handlers.",
            "Keep either `run:` or the inline `action:`",
        ),
        "ROUTE_BEHAVIOUR_REQUIRED" => (
            "Route has no behavior",
            "Every route needs exactly one executable behavior, supplied either by a named `run:` action invocation or an inline `action:` block.",
            "Add `run:` or an inline `action:`",
        ),
        "ROUTE_ITEM_COLON_REQUIRED" => (
            "Route item requires a `:` separator",
            "Every route item uses the explicit `name: value` form, including block-valued `path:` and `action:` items.",
            "Insert `:` after the route item name",
        ),
        "ROUTE_ITEM_DUPLICATE" => (
            "Route item is declared more than once",
            "A route has one authentication setting, path schema, input, output, and behavior selection. Repeating an item would silently replace part of its public contract.",
            "Keep one occurrence of this route item",
        ),
        "ROUTE_PATH_BINDING_DUPLICATE" => (
            "Route path binding name is repeated",
            "Each typed field in `path:` must bind one distinct placeholder; duplicate names cannot identify separate decoded path segments.",
            "Rename or remove the repeated path field",
        ),
        "ROUTE_PATH_BINDING_EXTRA" => (
            "Typed path binding has no placeholder",
            "Every typed field in `path:` must correspond to one same-named `{placeholder}` in the route path so callers actually supply its value.",
            "Remove the extra path field or add its matching placeholder",
        ),
        "ROUTE_PATH_BINDING_MISSING" => (
            "Route placeholder has no typed binding",
            "Every `{placeholder}` in a route path needs one same-named typed field in `path:` so the runtime can decode and validate it before the handler runs.",
            "Add a typed path field for the route placeholder",
        ),
        "ROUTE_PATH_FIELD_MODIFIER_INVALID" => (
            "Route path field has an unsupported modifier",
            "Path fields are required transport inputs and contain only a semantic type. Nullable, optional, constraint, reference, and persistence modifiers would give URL decoding storage semantics it does not have.",
            "Remove the modifier and keep only the required path-field type",
        ),
        "ROUTE_PATH_PLACEHOLDER_DUPLICATE" => (
            "Route path placeholder is repeated",
            "A placeholder name may occur once in a route path; repeating it would map one typed binding to multiple URL segments ambiguously.",
            "Give each route placeholder a distinct name",
        ),
        "ROUTE_PATH_PLACEHOLDER_INVALID" => (
            "Route path contains an invalid placeholder",
            "A placeholder uses `{name}` with a non-empty Jadpo name beginning with a letter or underscore and ending with a closing brace.",
            "Correct the placeholder to the `{name}` form",
        ),
        _ => return None,
    };
    Some(AuthoredCopy {
        summary,
        reason,
        next,
    })
}

fn data_catalogue_copy(code: &str) -> Option<AuthoredCopy> {
    let (summary, reason, next) = match code {
        "DATA_COMPOUND_CONSTRAINT_FIELDS" => (
            "Compound constraint needs at least two fields",
            "A named compound identity, uniqueness rule, or index describes a multi-field storage shape; a single field uses its ordinary field modifier instead.",
            "Add the other participating fields, or use a field-level modifier",
        ),
        "DATA_CONSTRAINT_DUPLICATE_FIELD" => (
            "Compound constraint repeats a field",
            "Each field may participate once in a compound constraint. Repetition does not add a column and makes the intended key shape unclear.",
            "Remove the repeated field from the constraint",
        ),
        "DATA_CONSTRAINT_NULLABLE_FIELD" => (
            "Compound constraint contains a nullable field",
            "Jadpo does not define cross-adapter uniqueness or identity semantics for null inside compound keys, so constrained fields must be non-nullable.",
            "Use non-nullable fields, or remove the field from the constraint",
        ),
        "DATA_CONSTRAINT_UNKNOWN_FIELD" => (
            "Compound constraint names an unknown field",
            "Every member of a compound constraint must resolve to a stored field on the same entity.",
            "Correct the field name or add the missing entity field",
        ),
        "DATA_DUPLICATE_CONSTRAINT_SHAPE" => (
            "Entity repeats the same constraint field set",
            "Two persistence constraints over the same ordered fields describe the same storage shape and would generate redundant or conflicting database objects.",
            "Keep one constraint for this field set",
        ),
        "DATA_IDENTITY_NULLABLE" => (
            "Entity identity field cannot be nullable",
            "Every stored entity instance needs a present, stable identity. A nullable identity could not address or reference every record.",
            "Make the identity field non-nullable",
        ),
        "DATA_INVERSE_DUPLICATE_NAME" => (
            "Inverse relationship name is already in use",
            "Fields and inverse relationships share an entity member namespace so selection and generated output paths resolve unambiguously.",
            "Rename the inverse relationship or conflicting field",
        ),
        "DATA_INVERSE_NOT_OWNING_REFERENCE" => (
            "Inverse relationship does not target an owning reference",
            "An inverse is derived from a stored reference field on the related entity; the named `via` field must be that owning reference.",
            "Point `via` at the related entity's owning reference field",
        ),
        "DATA_INVERSE_OPTIONAL_NOT_UNIQUE" => (
            "Optional inverse relationship is not unique",
            "An optional one-to-one inverse may return at most one record, so its owning reference must be protected by a unique constraint.",
            "Make the owning reference unique, or declare a many-valued inverse",
        ),
        "DATA_INVERSE_VIA_FIELD" => (
            "Inverse relationship names an unknown `via` field",
            "The `via` member must resolve to a stored field on the related entity before the compiler can derive the reverse relationship.",
            "Correct the `via` field name on the inverse relationship",
        ),
        "DATA_MODIFIER_NON_ENTITY" => (
            "Persistence modifier is only valid on entity fields",
            "Identity, uniqueness, indexing, and reference modifiers describe stored columns and have no meaning on value, input, or output records.",
            "Move the modifier to an entity field, or remove it",
        ),
        "DATA_MULTIPLE_IDENTITIES" => (
            "Entity declares more than one identity",
            "An entity has exactly one canonical identity used by references, generated persistence, and stable semantic IDs.",
            "Keep one identity field or one named identity constraint",
        ),
        "DATA_RELATIONSHIP_CYCLE" => (
            "Required cascading references form a delete cycle",
            "A cycle of required references with cascading deletion has no safe starting point and can recursively delete the same dependency graph.",
            "Break the cycle by changing one reference or its delete behavior",
        ),
        "DATA_RELATIONSHIP_SET_NULL_REQUIRED" => (
            "Required reference cannot use `set_null` on delete",
            "`set_null` clears the stored reference when its target is deleted, which is impossible for a non-nullable required field.",
            "Make the reference nullable or choose `restrict` or `cascade`",
        ),
        "DATA_RELATIONSHIP_TARGET_FIELD" => (
            "Reference target field does not exist",
            "A stored reference must name a real field on its target entity so generated foreign keys and nominal types share one identity.",
            "Correct the referenced target field",
        ),
        "DATA_RELATIONSHIP_TARGET_NOT_KEY" => (
            "Reference target is not an identity or unique key",
            "A relationship must point to a stable identity or unique field set; otherwise one stored reference could resolve to multiple target records.",
            "Reference the target identity or another unique key",
        ),
        "DATA_RELATIONSHIP_TYPE_MISMATCH" => (
            "Reference field type does not match its target",
            "The stored reference and referenced target field must have the same nominal semantic type so values cannot cross identifier domains.",
            "Use the referenced field's semantic type for this reference",
        ),
        _ => return None,
    };
    Some(AuthoredCopy {
        summary,
        reason,
        next,
    })
}

fn core_type_catalogue_copy(code: &str) -> Option<AuthoredCopy> {
    let (summary, reason, next) = match code {
        "TYPE_ARGUMENT_COUNT" => (
            "Call has the wrong number of arguments",
            "A function or action invocation must supply exactly one value for each declared parameter in declaration order.",
            "Add or remove arguments to match the callable signature",
        ),
        "TYPE_ARITHMETIC_OPERAND" => (
            "Arithmetic operator has a non-numeric operand",
            "Arithmetic operators accept compatible numeric values only; text, Boolean, records, and unrelated semantic types do not define arithmetic.",
            "Use numeric operands or choose an operation defined for these values",
        ),
        "TYPE_ASSIGN_IMMUTABLE" => (
            "Binding cannot be reassigned",
            "Parameters and ordinary `var` bindings are immutable. Only a local explicitly declared with `var mut` may receive a new value.",
            "Declare a mutable local when rebinding is intended, or create a new value",
        ),
        "TYPE_ASSIGN_UNKNOWN" => (
            "Assignment target does not exist",
            "Reassignment must target a visible local binding; declarations, fields, and unknown names cannot be assigned as locals.",
            "Correct the local name or declare it before assigning",
        ),
        "TYPE_CONSTRUCTOR_ARGUMENT_COUNT" => (
            "Semantic type constructor has the wrong number of arguments",
            "A scalar semantic type constructor wraps exactly one compatible underlying value before applying its declared constraints.",
            "Pass exactly one value to the semantic type constructor",
        ),
        "TYPE_CONSTRUCTOR_INPUT" => (
            "Semantic type constructor received an incompatible value",
            "The constructor input must be compatible with the semantic type's immediate parent; unrelated primitives or nominal siblings cannot be wrapped implicitly.",
            "Pass a value compatible with the type's parent",
        ),
        "TYPE_DUPLICATE_VARIANT_FIELD" => (
            "Enum variant payload field is repeated",
            "A data-carrying enum variant has one exact payload record; duplicate field names would make construction and pattern binding ambiguous.",
            "Keep one declaration of the variant payload field",
        ),
        "TYPE_FIELD_ON_NON_RECORD" => (
            "Field selection is applied to a non-record value",
            "Dot selection requires a value with a known record shape. Scalars, lists, and other non-record values do not expose named fields.",
            "Select from a record value or remove the field access",
        ),
        "TYPE_INCOMPARABLE" => (
            "Values cannot be compared for equality",
            "Equality requires compatible values from the same semantic domain or an explicitly permitted widening; unrelated nominal siblings are not comparable.",
            "Compare values from the same semantic type domain",
        ),
        "TYPE_INVALID_LITERAL" => (
            "Literal does not satisfy its semantic type",
            "The literal fails one or more constraints declared by the target semantic type, so constructing that value would violate its contract.",
            "Change the literal to satisfy the declared constraints",
        ),
        "TYPE_LOGICAL_OPERAND" => (
            "Logical operator requires Boolean operands",
            "`and` and `or` combine Boolean conditions only; Jadpo does not coerce numbers, text, records, or optional values to truthiness.",
            "Use Boolean expressions on both sides of the logical operator",
        ),
        "TYPE_MISMATCH" => (
            "Value is not compatible with the required type",
            "Jadpo preserves nominal semantic types and allows only defined widening conversions; it does not implicitly narrow primitives or cross sibling domains.",
            "Provide a value of the required semantic type or construct it explicitly",
        ),
        "TYPE_MISSING_FIELD" => (
            "Record construction is missing a required field",
            "A record constructor must provide every required field exactly once so the resulting value has the declaration's complete shape.",
            "Add the missing required field to the constructor",
        ),
        "TYPE_MISSING_VARIANT_FIELD" => (
            "Enum variant construction is missing a payload field",
            "A data-carrying variant constructor must provide every required field from that variant's exact payload shape.",
            "Add the missing field to the variant payload",
        ),
        "TYPE_NOT_RECORD" => (
            "Constructor target is not a record type",
            "Brace construction creates value, input, output, entity, failure, or variant record shapes; scalar semantic types use parenthesized construction.",
            "Use a record declaration with braces or a scalar constructor with parentheses",
        ),
        "TYPE_NULLABLE_SELECTION" => (
            "Nullable value must be narrowed before field selection",
            "A nullable record may be `none`, so selecting a field directly would read from a value whose record shape is not guaranteed to exist.",
            "Narrow the value with a nullable match before selecting its field",
        ),
        "TYPE_ORDERING_OPERAND" => (
            "Ordering operator has an unsupported operand",
            "Ordering is defined only for compatible ordered scalar values; records, Booleans, lists, and unrelated semantic domains have no stable ordering.",
            "Compare compatible ordered scalar values",
        ),
        "TYPE_PRIMITIVE_SIGNATURE" => (
            "Public callable signature uses a primitive type",
            "Callable parameters and results form semantic application contracts and must use named Jadpo types rather than unconstrained storage primitives.",
            "Introduce and use a named semantic type for this value",
        ),
        "TYPE_SIBLING_MISMATCH" => (
            "Value belongs to a different semantic type domain",
            "Types that share the same primitive parent remain nominal siblings. A value from one domain cannot stand in for another without explicit construction.",
            "Use the required sibling type or construct it from an allowed parent value",
        ),
        "TYPE_UNARY_OPERAND" => (
            "Unary operator has an incompatible operand",
            "Unary `not` requires Boolean and numeric negation requires a supported numeric value; other operand shapes do not define that operator.",
            "Use an operand supported by this unary operator",
        ),
        "TYPE_UNKNOWN_ENUM_VARIANT" => (
            "Enum variant does not exist",
            "The selected variant is not declared by this closed enum, so it cannot be constructed or matched.",
            "Use one of the enum's declared variants",
        ),
        "TYPE_UNKNOWN_FIELD" => (
            "Record field does not exist",
            "The record's exact declared shape has no field with this name; Jadpo does not add dynamic fields at runtime.",
            "Correct the field name or add it to the record declaration",
        ),
        "TYPE_UNKNOWN_NAME" => (
            "Type name does not exist",
            "The type reference does not resolve to a visible built-in, local declaration, or selected module import.",
            "Correct, declare, or import the referenced type",
        ),
        "TYPE_UNKNOWN_VALUE" => (
            "Value name does not exist in this scope",
            "The expression does not resolve to a local binding, parameter, supported literal, or visible value declaration.",
            "Correct the value name or declare it in the current scope",
        ),
        "TYPE_UNKNOWN_VARIANT_FIELD" => (
            "Enum variant payload field does not exist",
            "The data-carrying variant's exact payload shape has no field with this name.",
            "Correct the payload field name or declare it on the variant",
        ),
        _ => return None,
    };
    Some(AuthoredCopy {
        summary,
        reason,
        next,
    })
}

fn match_type_catalogue_copy(code: &str) -> Option<AuthoredCopy> {
    let (summary, reason, next) = match code {
        "TYPE_MATCH_DUPLICATE_BINDING" => (
            "Pattern binds the same payload field more than once",
            "Each enum payload field introduces one local binding in its match arm; repeating a name would create two bindings for the same value.",
            "Keep one binding for each payload field",
        ),
        "TYPE_MATCH_DUPLICATE_PATTERN" => (
            "Match pattern is already covered",
            "An earlier arm already handles this literal, enum variant, optional case, or wildcard, so this arm cannot select a new case.",
            "Remove the duplicate arm or change it to an uncovered case",
        ),
        "TYPE_MATCH_NON_EXHAUSTIVE" => (
            "Match does not cover every possible value",
            "Closed enums, Boolean values, and nullable cases must be handled exhaustively so execution always selects an arm when the match is reached.",
            "Add arms for every missing case",
        ),
        "TYPE_MATCH_PATTERN_TYPE" => (
            "Pattern cannot match the subject type",
            "This pattern belongs to a different enum or primitive value space than the expression being matched.",
            "Use a pattern from the subject's type",
        ),
        "TYPE_MATCH_SOME_NON_OPTIONAL" => (
            "`some` pattern requires a nullable value",
            "The `some(name)` pattern narrows a nullable value and binds its present case; a non-nullable subject is already known to be present.",
            "Match the value directly or make the subject nullable",
        ),
        "TYPE_MATCH_UNKNOWN_BINDING" => (
            "Pattern names an unknown variant payload field",
            "The selected enum variant has an exact payload shape, and this binding name is not one of its declared fields.",
            "Use a field declared by the selected variant",
        ),
        "TYPE_MATCH_UNKNOWN_VARIANT" => (
            "Pattern names an unknown enum variant",
            "The variant is not part of the matched enum's closed set, so no value of the subject type can select this arm.",
            "Use one of the matched enum's declared variants",
        ),
        "TYPE_MATCH_UNREACHABLE_PATTERN" => (
            "Match arm is unreachable after the wildcard",
            "A wildcard arm accepts every value not selected earlier, so no later pattern can ever run.",
            "Move the wildcard to the final arm or remove the unreachable arm",
        ),
        "TYPE_MATCH_VARIANT_BINDINGS_REQUIRED" => (
            "Variant pattern must bind its payload",
            "This enum variant carries fields. Matching only its name would discard the declared payload instead of introducing typed bindings for the arm.",
            "Add a binding list for the variant payload fields",
        ),
        "TYPE_MATCH_WILDCARD_REQUIRED" => (
            "Open value space requires a wildcard arm",
            "This subject is not a closed enum or Boolean space, so individual literal arms cannot prove that every possible value is covered.",
            "Add a final `_` arm for all remaining values",
        ),
        _ => return None,
    };
    Some(AuthoredCopy {
        summary,
        reason,
        next,
    })
}

fn persistence_type_catalogue_copy(code: &str) -> Option<AuthoredCopy> {
    let (summary, reason, next) = match code {
        "TYPE_CONFLICT_DUPLICATE_BINDING" => (
            "Constraint failure is mapped more than once",
            "Each named persistence constraint, and the optional fallback, may have only one conflict mapping so a database violation selects one domain failure.",
            "Keep one conflict mapping for this constraint",
        ),
        "TYPE_CONFLICT_UNKNOWN_CONSTRAINT" => (
            "Conflict mapping names an unknown constraint",
            "A specific conflict mapping must refer to a compiler-owned identity, unique, or named compound-unique constraint on the mutated entity.",
            "Use a declared persistence constraint from the target entity",
        ),
        "TYPE_CREATE_NOT_ENTITY" => (
            "Create target is not an entity",
            "`create` persists a declared entity shape; values, inputs, outputs, failures, and scalar types have no entity storage contract.",
            "Create a declared entity or use ordinary value construction",
        ),
        "TYPE_DELETE_NOT_ENTITY" => (
            "Delete target is not an entity",
            "`delete required` operates on persisted entity rows and cannot target a value, input, output, failure, or scalar type.",
            "Delete a declared entity",
        ),
        "TYPE_INCLUDE_DUPLICATE_RELATIONSHIP" => (
            "Relationship is included more than once",
            "A to-many query result has one field per included relationship; repeating a relationship would make that output field and its loading plan ambiguous.",
            "Keep one include for each relationship",
        ),
        "TYPE_INCLUDE_MIXED_CARDINALITY" => (
            "To-many include set contains a non-many relationship",
            "The batched child-include form combines only `many` inverse relationships. Owning-parent and optional inverse loads use their dedicated single-include forms.",
            "Use `many` for every include in this set or split out the single-related load",
        ),
        "TYPE_INCLUDE_ORDER_NOT_DETERMINISTIC" => (
            "Included children are not ordered by a stable key",
            "A bounded to-many include must order by an identity or unique child field so generated results cannot change arbitrarily between executions.",
            "Order the included relationship by an identity or unique field",
        ),
        "TYPE_INCLUDE_PARENT_PAGINATION_REQUIRED" => (
            "Many-parent include requires parent pagination",
            "Including children for an unbounded many-parent query could expand an unlimited parent set before child loading.",
            "Add a literal parent `limit` and optional non-negative `offset`",
        ),
        "TYPE_INCLUDE_REQUIRED_PARENT" => (
            "Optional parent query cannot include children",
            "The to-many include result shape contains a concrete parent value; an optional parent query could produce no parent to place in that shape.",
            "Use a required or many-parent query for this include",
        ),
        "TYPE_INCLUDE_RESULT_MISMATCH" => (
            "Includes use different result output types",
            "All relationships loaded by one query are assembled into one exact output shape and therefore must name the same `into` output type.",
            "Use the same result output type for every include",
        ),
        "TYPE_INCLUDE_RESULT_NOT_OUTPUT" => (
            "Include result type is not an output",
            "An include returns an explicitly declared boundary projection. Entities, values, and inputs cannot stand in for that named output contract.",
            "Declare and use an output type for the include result",
        ),
        "TYPE_INCLUDE_RESULT_SHAPE" => (
            "To-many include output has the wrong shape",
            "The result output must contain exactly `parent` with the queried entity type and one non-nullable `List<Child>` field for each included inverse relationship.",
            "Make the output fields exactly match the parent and included child lists",
        ),
        "TYPE_INCLUDE_UNKNOWN_RELATIONSHIP" => (
            "Included to-many relationship does not exist",
            "A to-many include must name a declared `inverse ... many` relationship on the queried parent entity.",
            "Use a declared many-valued inverse relationship",
        ),
        "TYPE_INVERSE_ONE_CARDINALITY" => (
            "Inverse-one include must be optional",
            "A zero-or-one inverse can have no related child, so its include cardinality and output field must preserve that possibility.",
            "Mark the inverse-one include `optional`",
        ),
        "TYPE_INVERSE_ONE_REQUIRED_QUERY" => (
            "Inverse-one include requires a parent value",
            "The inverse-one output contains a concrete parent and is only defined for a required parent query.",
            "Use `query required` for this inverse-one include",
        ),
        "TYPE_INVERSE_ONE_RESULT_SHAPE" => (
            "Inverse-one include output has the wrong shape",
            "The result output must contain exactly `parent` with the queried entity type and a nullable child field named for the optional inverse relationship.",
            "Make the output contain the exact parent and nullable inverse child fields",
        ),
        "TYPE_INVERSE_ONE_SINGLE" => (
            "Inverse-one query has more than one include",
            "The bounded zero-or-one loading plan supports exactly one optional inverse relationship per required parent query.",
            "Keep only one inverse-one include in this query",
        ),
        "TYPE_MUTATION_NULLABLE_FIELD_UNSUPPORTED" => (
            "Mutation predicate field is nullable",
            "The current required update and delete slice accepts a single equality predicate on a non-nullable entity field; nullable equality semantics are not implicit.",
            "Use a non-nullable predicate field",
        ),
        "TYPE_MUTATION_UNKNOWN_PREDICATE_FIELD" => (
            "Mutation predicate field does not exist",
            "The `where` field must be part of the target entity's declared record shape so its nominal type and storage column are known.",
            "Use a declared field from the target entity",
        ),
        "TYPE_NESTED_INCLUDE_CARDINALITY" => (
            "Nested include must be optional",
            "The supported second hop is a unique-backed optional inverse, so the final related value may be absent.",
            "Mark the nested include `optional`",
        ),
        "TYPE_NESTED_INCLUDE_FIRST_HOP" => (
            "Nested include first hop is not an owning reference",
            "The bounded two-hop form must first follow a declared owning reference from the queried child to its parent.",
            "Use a declared owning relationship for the first hop",
        ),
        "TYPE_NESTED_INCLUDE_NULLABLE_FIRST_HOP" => (
            "Nested include first hop is nullable",
            "The supported two-hop plan requires its intermediate parent to exist; a nullable owning reference cannot guarantee a value for the second lookup.",
            "Use a non-nullable owning reference for the first hop",
        ),
        "TYPE_NESTED_INCLUDE_REQUIRED_QUERY" => (
            "Nested include requires a parent value",
            "The bounded two-hop output contains the queried entity as a concrete `parent`, so it is defined only for a required query.",
            "Use `query required` for this nested include",
        ),
        "TYPE_NESTED_INCLUDE_RESULT_SHAPE" => (
            "Nested include output has the wrong shape",
            "The outer output must contain exactly the queried `parent` and the named first-hop field whose output, in turn, contains its parent and nullable second-hop child.",
            "Make both output layers match the exact two-hop result shape",
        ),
        "TYPE_NESTED_INCLUDE_SECOND_HOP" => (
            "Nested include second hop is not an optional inverse",
            "After the owning-reference hop, the supported bounded plan may follow only a declared unique-backed optional inverse relationship.",
            "Use an optional inverse relationship for the second hop",
        ),
        "TYPE_NESTED_INCLUDE_SINGLE" => (
            "Nested query has more than one include",
            "The bounded two-hop loading plan supports one explicit nested path and does not combine it with additional includes.",
            "Keep only the nested include in this query",
        ),
        "TYPE_PARENT_INCLUDE_NULLABLE_REFERENCE" => (
            "Required parent include follows a nullable reference",
            "A nullable owning reference may contain `none`, so the related parent cannot be promised as a non-nullable required output field.",
            "Mark the include optional or use a non-nullable reference",
        ),
        "TYPE_PARENT_INCLUDE_REQUIRED_QUERY" => (
            "Owning-parent include requires a child value",
            "The owning-parent output contains the queried child as a concrete `parent`, so it is defined only for a required child query.",
            "Use `query required` for this owning-parent include",
        ),
        "TYPE_PARENT_INCLUDE_RESULT_SHAPE" => (
            "Owning-parent include output has the wrong shape",
            "The result output must contain exactly `parent` with the queried child type and a relationship field whose nullability matches the include cardinality.",
            "Make the output contain the exact child and related-parent fields",
        ),
        "TYPE_PARENT_INCLUDE_SINGLE" => (
            "Owning-parent query has more than one include",
            "The bounded owning-reference loading plan supports exactly one related parent per required child query.",
            "Keep only one owning-parent include in this query",
        ),
        "TYPE_PARENT_INCLUDE_UNKNOWN_REFERENCE" => (
            "Included parent relationship is not an owning reference",
            "An owning-parent include must name a stored reference declared on the queried child entity, using its explicit relationship name when present.",
            "Use a declared owning relationship from the queried entity",
        ),
        "TYPE_PATCH_CONDITION_BINDING" => (
            "Patch supplied-condition uses the wrong binding",
            "A `when ... supplied` condition must select one field directly from the same patch input binding named by `patch:`.",
            "Use `<patch_binding>.<field> supplied` in the condition",
        ),
        "TYPE_PATCH_CONDITION_UNKNOWN_FIELD" => (
            "Patch supplied-condition names an unknown field",
            "The supplied flag exists only for fields declared by the patch input shape.",
            "Use a field declared on the patch input",
        ),
        "TYPE_PATCH_DERIVED_OVERLAP" => (
            "Patch and derived update write the same field",
            "A field cannot be written by both `patch:` and `set:` because the result would depend on an implicit update order.",
            "Remove the field from either the patch input or the derived `set` block",
        ),
        "TYPE_PATCH_FIELD_NOT_OPTIONAL" => (
            "Patch input field is not omission-aware",
            "Every patch input field must use the `optional` presence modifier so omission can be distinguished from supplying a value, including `none` for nullable fields.",
            "Mark every patch input field `optional`",
        ),
        "TYPE_PATCH_FIELD_REQUIRED" => (
            "Patch input has no fields",
            "A patch shape must declare at least one possible entity field; an empty input can never describe a write.",
            "Add at least one optional entity field to the patch input",
        ),
        "TYPE_PATCH_INPUT_BINDING" => (
            "Patch must name a direct input binding",
            "`patch:` accepts one local or parameter name so the compiler can track each field's supplied flag; field selections and longer paths are not patch bindings.",
            "Pass the patch input binding directly",
        ),
        "TYPE_PATCH_NOT_INPUT" => (
            "Patch binding is not an input record",
            "Omission metadata belongs to declared input fields. Values, outputs, entities, and scalar values do not carry the required supplied flags.",
            "Use a binding whose type is a declared input",
        ),
        "TYPE_PATCH_UNKNOWN_FIELD" => (
            "Patch input contains a field not on the entity",
            "Every patch field must map statically to a declared target-entity field so the generated update remains fixed and type checked.",
            "Remove the field or declare the corresponding entity field",
        ),
        "TYPE_QUERY_NOT_ENTITY" => (
            "Query target is not an entity",
            "`query` reads persisted entity rows; values, inputs, outputs, failures, and scalar types have no entity storage contract.",
            "Query a declared entity",
        ),
        "TYPE_QUERY_NULLABLE_FIELD_UNSUPPORTED" => (
            "Query predicate field is nullable",
            "The current query slice accepts a single equality predicate on a non-nullable field; nullable equality and `none` matching are not implicit.",
            "Use a non-nullable predicate field",
        ),
        "TYPE_QUERY_ORDER_NOT_DETERMINISTIC" => (
            "Query ordering field is not a stable key",
            "A many-result query must order by an identity or unique field so its row order and pagination boundary are deterministic.",
            "Order by an identity or unique field",
        ),
        "TYPE_QUERY_PAGINATION_CONSTANT_REQUIRED" => (
            "Pagination bound must be a literal integer",
            "Compile-time literal bounds keep the current query plan explicitly and statically bounded; dynamic limits and offsets are not part of this language slice.",
            "Use an integer literal for `limit` or `offset`",
        ),
        "TYPE_QUERY_PAGINATION_RANGE" => (
            "Pagination bound is outside its allowed range",
            "`limit` must be a positive integer and `offset` must be a non-negative integer.",
            "Use `limit` of at least 1 and `offset` of at least 0",
        ),
        "TYPE_QUERY_UNKNOWN_FIELD" => (
            "Query predicate field does not exist",
            "The `where` field must be part of the target entity's declared record shape so its nominal type and storage column are known.",
            "Use a declared field from the target entity",
        ),
        "TYPE_QUERY_UNKNOWN_ORDER_FIELD" => (
            "Query ordering field does not exist",
            "`order_by` must name a declared field on the queried entity before its stability can be checked.",
            "Use a declared field from the target entity",
        ),
        "TYPE_UPDATE_DUPLICATE_FIELD" => (
            "Update writes the same field more than once",
            "Each entity field may have only one fixed or conditional `set` write so the mutation has a single deterministic value for that column.",
            "Keep one `set` write for this field",
        ),
        "TYPE_UPDATE_FIELD_REQUIRED" => (
            "Update does not write any fields",
            "A required update must contain at least one fixed, conditional, or patch-derived field change; a predicate alone performs no mutation.",
            "Add a `set` change or a `patch` binding",
        ),
        "TYPE_UPDATE_NOT_ENTITY" => (
            "Update target is not an entity",
            "`update required` mutates persisted entity rows and cannot target a value, input, output, failure, or scalar type.",
            "Update a declared entity",
        ),
        "TYPE_UPDATE_UNKNOWN_FIELD" => (
            "Updated field does not exist on the entity",
            "Every fixed or conditional `set` entry must name a declared target-entity field so its nominal type and storage column are known.",
            "Use a declared field from the target entity",
        ),
        _ => return None,
    };
    Some(AuthoredCopy {
        summary,
        reason,
        next,
    })
}

fn module_catalogue_copy(code: &str) -> Option<AuthoredCopy> {
    let (summary, reason, next) = match code {
        "MOD_DUPLICATE_IMPORT" => (
            "Declaration is imported more than once",
            "A module scope may bind an imported declaration name only once; repeated imports do not create a second distinct value or type.",
            "Keep one import of this declaration",
        ),
        "MOD_DUPLICATE_MODULE" => (
            "Module name is declared by more than one file",
            "Each explicit module has one source file so its exports, imports, and declaration scope have a single owner.",
            "Give each module file a unique module name",
        ),
        "MOD_IMPORT_CONFLICT" => (
            "Imported name conflicts with a local declaration",
            "The module already declares this name locally, so importing another declaration under the same name would make references ambiguous.",
            "Remove the import or rename one of the declarations",
        ),
        "MOD_IMPORT_CYCLE" => (
            "Module imports form a cycle",
            "Modules must have an acyclic dependency order so declarations can be resolved and generated deterministically.",
            "Move shared declarations or remove an import to break the cycle",
        ),
        "MOD_IMPORT_REQUIRED" => (
            "Declaration is used without being imported",
            "In explicit-module mode, declarations from another file are visible only when selected by an import in the current module.",
            "Import the declaration from its module",
        ),
        "MOD_MODULE_REQUIRED" => (
            "Source file is missing its module declaration",
            "Once any source uses explicit modules, every source file must declare its module so cross-file visibility is deterministic.",
            "Add a module declaration to this file",
        ),
        "MOD_PRIVATE_IMPORT" => (
            "Imported declaration is private to its module",
            "The target declaration exists but is not listed as public by its owning module, so another module cannot select it.",
            "Export the declaration from its module or stop importing it",
        ),
        "MOD_PUBLIC_REQUIRES_MODULE" => (
            "Public export requires explicit modules",
            "A public declaration needs an owning module boundary; export markers cannot be used while the project is in implicit single-scope mode.",
            "Declare modules for the project or remove the public export",
        ),
        "MOD_SELF_IMPORT" => (
            "Module imports itself",
            "Declarations in the current module are already in scope, and a self-import would create a dependency cycle without adding visibility.",
            "Remove the self-import",
        ),
        "MOD_UNKNOWN_EXPORT" => (
            "Imported declaration does not exist in the target module",
            "The selected name is neither a public export nor a private declaration in the module being imported.",
            "Correct the imported name or add and export that declaration",
        ),
        "MOD_UNKNOWN_MODULE" => (
            "Imported module does not exist",
            "No source file declares the module named by this import, so its selected declarations cannot be resolved.",
            "Correct the module path or add the missing module file",
        ),
        _ => return None,
    };
    Some(AuthoredCopy {
        summary,
        reason,
        next,
    })
}

fn tooling_catalogue_copy(code: &str) -> Option<AuthoredCopy> {
    let (summary, reason, next) = match code {
        "FMT_CHANGES_REQUIRED" => (
            "Source files are not canonically formatted",
            "The formatter check found source whose canonical Jadpo layout differs from the checked-in text.",
            "Run `jadpo fmt` and review the resulting source changes",
        ),
        "FMT_WRITE_FAILED" => (
            "Formatted source could not be written",
            "The formatter produced canonical text but the source file could not be replaced, commonly because of filesystem permissions or an I/O failure.",
            "Make the source writable and run the formatter again",
        ),
        "LSP_CONTENT_LENGTH_MISSING" => (
            "Language-server message has no Content-Length header",
            "The editor connection sent an LSP frame without the byte length required to read exactly one JSON-RPC message.",
            "Restart the editor connection and check the LSP client transport",
        ),
        "LSP_JSON_INVALID" => (
            "Language-server message is not valid JSON",
            "The editor connection delivered a complete frame whose body could not be decoded as a JSON-RPC message.",
            "Restart the editor connection and inspect the client message payload",
        ),
        "LSP_JSON_WRITE_FAILED" => (
            "Language-server response could not be encoded",
            "The server produced a response that could not be serialized as JSON for the editor connection.",
            "Restart the language server and report the request that triggered this response",
        ),
        "LSP_READ_FAILED" => (
            "Language-server input could not be read",
            "The editor connection closed or failed while the server was reading an LSP header or message body.",
            "Restart the editor connection; if it recurs, inspect the client transport logs",
        ),
        "LSP_ROOT_MISSING" => (
            "Language server has no workspace root",
            "The editor did not provide a workspace folder or root URI, so the server cannot discover the Jadpo project and its source files.",
            "Open the Jadpo project folder as an editor workspace and restart the server",
        ),
        "LSP_WRITE_FAILED" => (
            "Language-server response could not be written",
            "The editor connection closed or failed while the server was sending a framed JSON-RPC response.",
            "Restart the editor connection; if it recurs, inspect the client transport logs",
        ),
        _ => return None,
    };
    Some(AuthoredCopy {
        summary,
        reason,
        next,
    })
}

fn index_catalogue_copy(code: &str) -> Option<AuthoredCopy> {
    let (summary, reason, next) = match code {
        "INDEX_ACCEPT_CHECK_FAILED" => (
            "Index edit did not pass project checks",
            "After inserting the recommended `index` modifier, the compiler found diagnostics and refused to retain an unchecked source change.",
            "Review the project diagnostics before accepting the recommendation again",
        ),
        "INDEX_ACCEPT_IDENTITY_COUNT" => (
            "Index acceptance changed an unexpected number of schema identities",
            "Accepting one recommendation must register exactly one new index identity; any other count means the source and identity registry are not changing atomically as expected.",
            "Restore schema identity consistency and retry the single recommendation",
        ),
        "INDEX_ACCEPT_RANGE_INVALID" => (
            "Recommended index insertion point is no longer valid",
            "The recorded field range does not match the current source text, so inserting `index` could edit the wrong bytes.",
            "Re-run the index recommendation command against the current sources",
        ),
        "INDEX_ACCEPT_REGISTRY_MISSING" => (
            "Schema identity registry is required before accepting an index",
            "The accepted index becomes durable schema identity and cannot be written safely until the project registry has been initialized.",
            "Run `jadpo schema init` for this project, then accept the index again",
        ),
        "INDEX_ACCEPT_ROLLBACK_FAILED" => (
            "Failed index edit could not be rolled back",
            "The compiler rejected the edited project but could not restore the original source file, so manual recovery is required before another attempt.",
            "Restore the source from version control and inspect filesystem permissions",
        ),
        "INDEX_ACCEPT_SOURCE_READ_FAILED" => (
            "Source file for the recommended index could not be read",
            "The recommendation resolved to a source file that became unavailable before the compiler could prepare its checked edit.",
            "Restore read access to the source and re-run the recommendation",
        ),
        "INDEX_ACCEPT_SOURCE_WRITE_FAILED" => (
            "Recommended index could not be written to source",
            "The compiler prepared the checked `index` insertion but the source file could not be updated.",
            "Make the source writable and accept the recommendation again",
        ),
        "INDEX_RECOMMENDATION_AVAILABLE" => (
            "A query field could benefit from an index",
            "Static query analysis found a predicate or ordering field without a matching persistence index; this is advice and does not change source automatically.",
            "Inspect the recommendation and explicitly accept it if the storage trade-off is appropriate",
        ),
        "INDEX_RECOMMENDATION_FIELD_MISSING" => (
            "Recommended field no longer exists",
            "The selected recommendation refers to an entity field that is absent from the currently analysed sources.",
            "Re-run index recommendation and choose an item from the current results",
        ),
        "INDEX_RECOMMENDATION_UNKNOWN" => (
            "Index recommendation is not current",
            "The requested entity-field recommendation is not present in the latest static query evidence for this project.",
            "List current index recommendations and choose one of their identifiers",
        ),
        _ => return None,
    };
    Some(AuthoredCopy {
        summary,
        reason,
        next,
    })
}

fn toolchain_catalogue_copy(code: &str) -> Option<AuthoredCopy> {
    let (summary, reason, next) = match code {
        "JADPO_ARTIFACT_CLEANUP_FAILED" => (
            "Previous build backup could not be removed",
            "The new artifact set was promoted successfully, but cleanup of the temporary backup directory failed.",
            "Inspect permissions on the project build directories and remove the stale backup safely",
        ),
        "JADPO_ARTIFACT_PROMOTE_FAILED" => (
            "Staged build artifacts could not be promoted",
            "The compiler generated a complete staging directory but could not atomically replace the project build output; the previous complete output was preserved when possible.",
            "Restore write and rename access to the project directory, then build again",
        ),
        "JADPO_ARTIFACT_STAGE_FAILED" => (
            "Build staging directory could not be created",
            "Artifact generation writes to a project-local staging directory before promotion so an incomplete build never replaces the last complete output.",
            "Make the project directory writable and build again",
        ),
        "JADPO_ARTIFACT_WRITE_FAILED" => (
            "Generated artifact could not be written",
            "The compiler could not create a directory or file inside its isolated build staging area, so the staged output was discarded.",
            "Check available space and project-directory permissions, then build again",
        ),
        "JADPO_EMPTY_PROJECT" => (
            "Project contains no declarations",
            "Jadpo source files were found and parsed without syntax errors, but none declares a type, enum, record, failure, callable, test, or route to compile.",
            "Add at least one top-level Jadpo declaration",
        ),
        "JADPO_NO_SOURCES" => (
            "Project contains no Jadpo source files",
            "Source discovery found no files ending in `.jadpo` beneath the requested project path, excluding generated build output.",
            "Add a `.jadpo` source file or select the correct project path",
        ),
        "JADPO_PROJECT_NOT_FOUND" => (
            "Project path does not exist",
            "The requested file or directory was not present when Jadpo began source discovery.",
            "Correct the project path or create the project first",
        ),
        "JADPO_PROJECT_READ_FAILED" => (
            "Project directory could not be read",
            "Source discovery could not enumerate part of the project tree, so it cannot know which Jadpo files belong to the build.",
            "Restore directory read access and try again",
        ),
        "JADPO_SCAFFOLD_DESTINATION_EXISTS" => (
            "Scaffold destination is an existing file",
            "A new Jadpo project needs a directory destination; the selected path already belongs to a non-directory filesystem entry.",
            "Choose a new directory path for the project",
        ),
        "JADPO_SCAFFOLD_DESTINATION_NOT_EMPTY" => (
            "Scaffold destination directory is not empty",
            "Project creation refuses to overwrite or mix generated starter files with existing directory contents.",
            "Choose an empty directory or a new destination",
        ),
        "JADPO_SCAFFOLD_NAME_INVALID" => (
            "Project destination has no valid name",
            "The scaffold manifest derives its project name from the final destination path component, which is missing or not valid text.",
            "Choose a destination path with a valid final directory name",
        ),
        "JADPO_SCAFFOLD_READ_FAILED" => (
            "Scaffold destination could not be inspected",
            "The project creator could not read the destination directory to prove that writing starter files would not overwrite existing content.",
            "Restore read access or choose another destination",
        ),
        "JADPO_SCAFFOLD_WRITE_FAILED" => (
            "Project scaffold file could not be written",
            "The project creator could not create one of the required starter directories or files.",
            "Restore write access and available space, then create the project again",
        ),
        "JADPO_SOURCE_READ_FAILED" => (
            "Jadpo source file could not be read",
            "Source discovery found the file, but its text became unavailable before parsing and checking could begin.",
            "Restore read access to the source file and retry",
        ),
        "JADPO_TARGET_AUTH_NOT_IMPLEMENTED" => (
            "Protected route cannot be generated yet",
            "The current target runtime can generate explicitly public `auth: none` routes, but no accepted authentication provider boundary exists for protected routes, so generation stops rather than weakening access control.",
            "Choose and implement the authentication boundary before generating this protected route",
        ),
        "JADPO_TARGET_DEPENDENCY_MANIFEST" => (
            "Generated target attempted to add a dependency manifest",
            "Jadpo's generated runtime must remain self-contained and may not emit package manifests, lockfiles, or vendored dependency directories.",
            "Remove the generated dependency artifact from the target generator",
        ),
        "JADPO_TARGET_EXTERNAL_MODULE" => (
            "Generated target imports an external module",
            "Generated TypeScript may use the Bun runtime and compiler-emitted relative modules only; undeclared external packages would make the output non-hermetic.",
            "Replace the import with supported runtime functionality or a generated relative module",
        ),
        _ => return None,
    };
    Some(AuthoredCopy {
        summary,
        reason,
        next,
    })
}

pub fn catalogue_definition(code: &str) -> CatalogueDefinition {
    let (category, remainder) = code.split_once('_').unwrap_or(("diagnostic", code));
    let category = match category {
        "SYN" => "syntax",
        "SEM" => "semantic",
        "TYPE" => "type",
        "FAIL" => "failure",
        "EFFECT" => "effect",
        "ROUTE" => "route",
        "DATA" => "data",
        "MOD" => "module",
        "MIG" => "migration",
        "INDEX" => "index",
        "CLI" => "cli",
        "LSP" => "lsp",
        "FMT" => "format",
        "RUNTIME" => "runtime",
        "TEST" => "test",
        "JADPO" => "toolchain",
        other => other,
    }
    .to_ascii_lowercase();
    let rule_id = format!("{category}.{}", remainder.to_ascii_lowercase());
    let authored = syntax_catalogue_copy(code)
        .or_else(|| semantic_catalogue_copy(code))
        .or_else(|| failure_catalogue_copy(code))
        .or_else(|| route_catalogue_copy(code))
        .or_else(|| data_catalogue_copy(code))
        .or_else(|| core_type_catalogue_copy(code))
        .or_else(|| match_type_catalogue_copy(code))
        .or_else(|| persistence_type_catalogue_copy(code))
        .or_else(|| module_catalogue_copy(code))
        .or_else(|| tooling_catalogue_copy(code))
        .or_else(|| index_catalogue_copy(code))
        .or_else(|| toolchain_catalogue_copy(code));
    let human_owned = matches!(
        code,
        "JADPO_TARGET_AUTH_NOT_IMPLEMENTED"
            | "ROUTE_AUTH_VALUE_INVALID"
            | "MIG_DECISION_UNRESOLVED"
            | "MIG_DECISION_MISSING"
            | "MIG_DECISION_EVIDENCE_MISSING"
            | "MIG_PLAN_DECISION_REJECTS_CHANGE"
    );
    let automatic = matches!(
        code,
        "FAIL_ATTEMPT_REQUIRED" | "FAIL_STALE_DECLARATION" | "ROUTE_ITEM_COLON_REQUIRED"
    );
    let (repair_kind, decision_owner, recommended_title) = if human_owned {
        (
            RepairKind::HumanDecision,
            DecisionOwner::Human,
            match code {
                "ROUTE_AUTH_VALUE_INVALID" => "Choose the authentication boundary for `{route}`",
                "JADPO_TARGET_AUTH_NOT_IMPLEMENTED" => {
                    "Choose and implement the authentication boundary"
                }
                _ => "Request the named human-owned decision",
            }
            .to_owned(),
        )
    } else if automatic {
        (
            RepairKind::AutomaticFix,
            DecisionOwner::Compiler,
            match code {
                "FAIL_ATTEMPT_REQUIRED" => "Prefix the fallible expression with `attempt`",
                "FAIL_STALE_DECLARATION" => "Remove the stale `fails` entry",
                "ROUTE_ITEM_COLON_REQUIRED" => "Insert `:` after the route item name",
                _ => "Apply the compiler-verified edit",
            }
            .to_owned(),
        )
    } else {
        (
            RepairKind::GuidedChoice,
            DecisionOwner::Agent,
            match code {
                "FAIL_CONTEXT_FIELD_OVERLAP" => {
                    "Choose one disclosure class for the failure context field"
                }
                "ROUTE_PATH_BINDING_MISSING" => "Add a typed path field for the route placeholder",
                "ROUTE_PATH_BINDING_EXTRA" => "Remove the typed path field that has no placeholder",
                "ROUTE_BEHAVIOUR_CONFLICT" => "Keep either `run:` or the inline `action:`",
                "ROUTE_BEHAVIOUR_REQUIRED" => "Add `run:` or an inline `action:`",
                "CLI_INCIDENT_REVISION_MISMATCH" => {
                    "Use the checked sources that produced the runtime event"
                }
                "CLI_PRESENTATION_ARGUMENTS" => {
                    "Choose one supported diagnostic format and colour mode"
                }
                "RUNTIME_UNHANDLED_FAULT" => {
                    "Inspect the matching local incident using its request identifier"
                }
                "RUNTIME_STARTUP_FAILED" => {
                    "Inspect the generated runtime startup event before retrying"
                }
                "SYN_EXPECTED_DECLARATION" => {
                    "Move `{found}` into its owning declaration, or replace it with a top-level declaration"
                }
                "SYN_UNEXPECTED_TOKEN" => "Provide {expected}",
                _ => authored
                    .map(|copy| copy.next)
                    .unwrap_or("Update the source to satisfy this rule"),
            }
            .to_owned(),
        )
    };
    let summary = match code {
        "FAIL_ATTEMPT_REQUIRED" => "Fallible expression requires `attempt`".to_owned(),
        "FAIL_STALE_DECLARATION" => "Declared problem is not reachable".to_owned(),
        "FAIL_CONTEXT_FIELD_OVERLAP" => {
            "Failure context field has conflicting disclosure".to_owned()
        }
        "ROUTE_PATH_BINDING_MISSING" => "Route placeholder has no typed binding".to_owned(),
        "ROUTE_PATH_BINDING_EXTRA" => "Typed path binding has no placeholder".to_owned(),
        "ROUTE_BEHAVIOUR_CONFLICT" => "Route declares two behaviour forms".to_owned(),
        "ROUTE_BEHAVIOUR_REQUIRED" => "Route has no behaviour".to_owned(),
        "ROUTE_ITEM_COLON_REQUIRED" => "Route item requires a `:` separator".to_owned(),
        "ROUTE_AUTH_VALUE_INVALID" => {
            "Route authentication value `{found}` is not valid".to_owned()
        }
        "CLI_INCIDENT_REVISION_MISMATCH" => {
            "Runtime event and local source revisions differ".to_owned()
        }
        "CLI_PRESENTATION_ARGUMENTS" => "Terminal presentation option is invalid".to_owned(),
        "RUNTIME_UNHANDLED_FAULT" => "Generated runtime contained an unexpected fault".to_owned(),
        "RUNTIME_STARTUP_FAILED" => "Generated runtime failed during startup".to_owned(),
        "SYN_EXPECTED_DECLARATION" => "`{found}` cannot start a top-level declaration".to_owned(),
        "SYN_UNEXPECTED_TOKEN" => "Expected {expected}".to_owned(),
        _ => authored
            .map(|copy| copy.summary.to_owned())
            .unwrap_or_else(|| sentence_case_identifier(remainder)),
    };
    let reason = match code {
        "FAIL_ATTEMPT_REQUIRED" => "Every expression with a recoverable problem set must make propagation visible with `attempt`.".to_owned(),
        "FAIL_STALE_DECLARATION" => "A callable's authored `fails` set must exactly equal its reachable unhandled problem set.".to_owned(),
        "FAIL_CONTEXT_FIELD_OVERLAP" => "A flat failure value cannot be assigned safely when the declaration gives the same field both public and internal disclosure.".to_owned(),
        "ROUTE_PATH_BINDING_MISSING" | "ROUTE_PATH_BINDING_EXTRA" => "Route placeholders and typed `path` fields must correspond exactly one-to-one.".to_owned(),
        "ROUTE_BEHAVIOUR_CONFLICT" | "ROUTE_BEHAVIOUR_REQUIRED" => "A route must select exactly one local inline action or one named `run:` invocation.".to_owned(),
        "ROUTE_ITEM_COLON_REQUIRED" => "Every route item uses the same explicit `name: value` separator, including block-valued path and action items.".to_owned(),
        "ROUTE_AUTH_VALUE_INVALID" => "Routes require authentication by default. The only explicit opt-out is the exact form `auth: none`, so the compiler cannot infer whether `{found}` was meant to retain authentication or disable it.".to_owned(),
        "CLI_INCIDENT_REVISION_MISMATCH" => "Local enrichment is trustworthy only when the runtime event and compiler graph identify the same checked source revision.".to_owned(),
        "CLI_PRESENTATION_ARGUMENTS" => "Diagnostic format and colour flags must select one supported presentation without changing the semantic diagnostic payload.".to_owned(),
        "RUNTIME_UNHANDLED_FAULT" => "An exception outside the declared domain-failure boundary was contained by the generated runtime.".to_owned(),
        "RUNTIME_STARTUP_FAILED" => "The generated runtime could not establish its startup contract and did not report readiness.".to_owned(),
        "SYN_EXPECTED_DECLARATION" => "Jadpo files accept `type`, `enum`, `entity`, `value`, `input`, `output`, `failure`, `function`, `action`, `test`, and `route` declarations at the top level. Route items such as `path:` belong inside a `route` block, so `{found}` cannot be parsed here.".to_owned(),
        "SYN_UNEXPECTED_TOKEN" => "Found `{found}` while parsing this construct. Jadpo requires {expected} at this location, so parsing stops rather than guessing the authored structure.".to_owned(),
        _ => authored.map_or_else(
            || format!("The compiler-enforced `{code}` invariant is not satisfied at this location."),
            |copy| copy.reason.to_owned(),
        ),
    };
    let mut fixtures = CATALOGUE_FIXTURES
        .iter()
        .filter_map(|(fixture_code, path)| (*fixture_code == code).then_some(*path))
        .collect::<Vec<_>>();
    if fixtures.is_empty() {
        fixtures.push("jadpo/crates/diagnostics/src/lib.rs#every_catalogue_entry_is_renderable");
    }
    let authored_copy = authored.is_some()
        || matches!(
            code,
            "FAIL_ATTEMPT_REQUIRED"
                | "FAIL_STALE_DECLARATION"
                | "FAIL_CONTEXT_FIELD_OVERLAP"
                | "ROUTE_PATH_BINDING_MISSING"
                | "ROUTE_PATH_BINDING_EXTRA"
                | "ROUTE_BEHAVIOUR_CONFLICT"
                | "ROUTE_BEHAVIOUR_REQUIRED"
                | "ROUTE_ITEM_COLON_REQUIRED"
                | "ROUTE_AUTH_VALUE_INVALID"
                | "CLI_INCIDENT_REVISION_MISMATCH"
                | "CLI_PRESENTATION_ARGUMENTS"
                | "RUNTIME_UNHANDLED_FAULT"
                | "RUNTIME_STARTUP_FAILED"
                | "SYN_EXPECTED_DECLARATION"
                | "SYN_UNEXPECTED_TOKEN"
        );
    CatalogueDefinition {
        help_id: format!("diagnostics/{rule_id}"),
        rule_id,
        category,
        summary,
        reason,
        recommended_title,
        repair_kind,
        decision_owner,
        context_keys: match code {
            "FAIL_STALE_DECLARATION" => vec!["callable", "declared", "reachable"],
            "CLI_INCIDENT_REVISION_MISMATCH" => vec!["eventRevision", "localRevision"],
            "ROUTE_AUTH_VALUE_INVALID" => vec!["route", "found"],
            "SYN_EXPECTED_DECLARATION" => vec!["found"],
            "SYN_UNEXPECTED_TOKEN" => vec!["expected", "found"],
            _ => Vec::new(),
        },
        fixtures,
        authored_copy,
    }
}

fn render_catalogue_text(template: &str, context: &[(String, String)]) -> String {
    context
        .iter()
        .fold(template.to_owned(), |rendered, (key, value)| {
            rendered.replace(&format!("{{{key}}}"), value)
        })
}

fn sentence_case_identifier(value: &str) -> String {
    let mut words = value
        .split('_')
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    if let Some(first) = words.first_mut() {
        if let Some(initial) = first.get_mut(0..1) {
            initial.make_ascii_uppercase();
        }
    }
    words.join(" ")
}

fn default_alternatives(code: &str, _definition: &CatalogueDefinition) -> Vec<RepairStep> {
    if code == "ROUTE_AUTH_VALUE_INVALID" {
        return vec![
            RepairStep {
                kind: RepairKind::HumanDecision,
                title: "Keep authentication required for this route".to_owned(),
                reason: "Remove the invalid `auth:` item so the route keeps Jadpo's authenticated default.".to_owned(),
                decision_owner: DecisionOwner::Human,
                preferred: false,
                edits: Vec::new(),
                behavioral_effect: "The route continues to require authentication.".to_owned(),
                public_contract_effect: "The public route remains authenticated. Target generation may still be blocked until authentication runtime support is configured.".to_owned(),
            },
            RepairStep {
                kind: RepairKind::HumanDecision,
                title: "Make this route explicitly unauthenticated".to_owned(),
                reason: "Replace the invalid value with the exact opt-out `none` only when unauthenticated access is intentional.".to_owned(),
                decision_owner: DecisionOwner::Human,
                preferred: false,
                edits: Vec::new(),
                behavioral_effect: "Requests may reach the route without an authenticated identity.".to_owned(),
                public_contract_effect: "The public security contract changes: this route becomes callable without authentication.".to_owned(),
            },
        ];
    }
    if code == "SYN_EXPECTED_DECLARATION" {
        return vec![RepairStep {
            kind: RepairKind::GuidedChoice,
            title: "Remove the stray top-level construct".to_owned(),
            reason: "Delete it only when it is leftover text rather than content intended for a declaration or route.".to_owned(),
            decision_owner: DecisionOwner::Agent,
            preferred: false,
            edits: Vec::new(),
            behavioral_effect: "The stray construct no longer contributes authored behavior.".to_owned(),
            public_contract_effect: "Review whether the removed construct was intended to change a route or another public declaration.".to_owned(),
        }];
    }
    let (title, reason) = match code {
        "FAIL_STALE_DECLARATION" => (
            "Add the missing reachable behavior intentionally",
            "Keep the declared problem only by adding a reviewed path that can actually produce it.",
        ),
        "ROUTE_PATH_BINDING_MISSING" => (
            "Remove or rename the route placeholder",
            "Change the public path template when that segment is not intended to be a typed input.",
        ),
        "ROUTE_PATH_BINDING_EXTRA" => (
            "Add the matching route placeholder",
            "Change the public path template when the typed field is intended to be externally supplied.",
        ),
        _ => return Vec::new(),
    };
    vec![RepairStep {
        kind: RepairKind::GuidedChoice,
        title: title.to_owned(),
        reason: reason.to_owned(),
        decision_owner: DecisionOwner::Agent,
        preferred: false,
        edits: Vec::new(),
        behavioral_effect: "This alternative changes authored behavior and requires validation."
            .to_owned(),
        public_contract_effect: if code.starts_with("ROUTE_") {
            "This alternative changes the public route contract.".to_owned()
        } else {
            "Review generated contract impact before applying this alternative.".to_owned()
        },
    }]
}

pub fn catalogue_manifest_json() -> String {
    let entries = CATALOGUE_CODES
        .iter()
        .map(|code| {
            let entry = catalogue_definition(code);
            let allowed_keys = entry
                .context_keys
                .iter()
                .map(|key| json_string(key))
                .collect::<Vec<_>>()
                .join(",");
            let fixtures = entry
                .fixtures
                .iter()
                .map(|fixture| json_string(fixture))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"ruleId\":{},\"category\":{},\"summary\":{},\"reason\":{},\"recommendedNextStep\":{},\"repairKind\":{},\"decisionOwner\":{},\"copyStatus\":{},\"contextSchema\":{{\"allowedKeys\":[{allowed_keys}],\"additionalProperties\":false}},\"helpId\":{},\"fixtures\":[{fixtures}],\"legacyAliases\":[{}]}}",
                json_string(&entry.rule_id),
                json_string(&entry.category),
                json_string(&entry.summary),
                json_string(&entry.reason),
                json_string(&entry.recommended_title),
                json_string(entry.repair_kind.as_str()),
                json_string(entry.decision_owner.as_str()),
                json_string(if entry.authored_copy { "authored" } else { "placeholder" }),
                json_string(&entry.help_id),
                json_string(code),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"schemaVersion\":1,\"diagnostics\":[{entries}]}}")
}

pub fn catalogue_reference_markdown() -> String {
    let mut output = String::from(
        "# Compiler diagnostic catalogue\n\nGenerated from the compiler-owned catalogue.\n\n",
    );
    for code in CATALOGUE_CODES {
        let entry = catalogue_definition(code);
        output.push_str(&format!(
            "## `{}`\n\n{}\n\n{}\n\nRecommended next step: {} (`{}`, owner `{}`).\n\nHelp ID: `{}`. Legacy alias: `{}`. Fixtures: {}.\n\n",
            entry.rule_id,
            entry.summary,
            entry.reason,
            entry.recommended_title,
            entry.repair_kind.as_str(),
            entry.decision_owner.as_str(),
            entry.help_id,
            code,
            if entry.fixtures.is_empty() { "none".to_owned() } else { entry.fixtures.join(", ") }
        ));
    }
    output
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublicFailureValue {
    Text(String),
    Integer(i64),
    Boolean(bool),
    None,
}

impl PublicFailureValue {
    fn to_json(&self) -> String {
        match self {
            Self::Text(value) => json_string(value),
            Self::Integer(value) => value.to_string(),
            Self::Boolean(value) => value.to_string(),
            Self::None => "null".to_owned(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SafeIdentifier(String);

impl SafeIdentifier {
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        (!value.is_empty()
            && value.len() <= 256
            && value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric()
                    || matches!(byte, b'_' | b'-' | b':' | b'/' | b'{' | b'}' | b'.')
            }))
        .then_some(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationalValue {
    StaticText(&'static str),
    Identifier(SafeIdentifier),
    Integer(i64),
    Boolean(bool),
    None,
}

impl OperationalValue {
    fn to_json(&self) -> String {
        match self {
            Self::StaticText(value) => json_string(value),
            Self::Identifier(value) => json_string(value.as_str()),
            Self::Integer(value) => value.to_string(),
            Self::Boolean(value) => value.to_string(),
            Self::None => "null".to_owned(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentContextValue {
    CompilerText(String),
    Identifier(SafeIdentifier),
    Integer(i64),
    Boolean(bool),
    None,
}

pub struct Secret<T>(T);

impl<T> Secret<T> {
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    pub fn expose_to_boundary(&self) -> &T {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicFailureResponse {
    pub code: String,
    pub message: String,
    pub request_id: String,
    pub details: Vec<(String, PublicFailureValue)>,
}

impl PublicFailureResponse {
    pub fn to_json(&self) -> String {
        let details = self
            .details
            .iter()
            .map(|(key, value)| format!("{}:{}", json_string(key), value.to_json()))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"error\":{{\"code\":{},\"message\":{},\"request_id\":{},\"details\":{{{details}}}}}}}",
            json_string(&self.code),
            json_string(&self.message),
            json_string(&self.request_id),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationalLogEvent {
    pub event_name: String,
    pub classification: String,
    pub request_id: String,
    pub trace_id: Option<String>,
    pub semantic_operation_id: String,
    pub source_revision: String,
    pub safe_attributes: Vec<(String, OperationalValue)>,
}

impl OperationalLogEvent {
    pub fn to_json(&self) -> String {
        let attributes = self
            .safe_attributes
            .iter()
            .take(16)
            .map(|(key, value)| format!("{}:{}", json_string(key), value.to_json()))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"schemaVersion\":1,\"kind\":\"operational_log_event\",\"eventName\":{},\"classification\":{},\"requestId\":{},\"traceId\":{},\"semanticOperationId\":{},\"sourceRevision\":{},\"attributes\":{{{attributes}}}}}",
            json_string(&self.event_name),
            json_string(&self.classification),
            json_string(&self.request_id),
            self.trace_id.as_deref().map(json_string).unwrap_or_else(|| "null".to_owned()),
            json_string(&self.semantic_operation_id),
            json_string(&self.source_revision),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentIncidentPacket {
    pub source_revision: String,
    pub semantic_operation_id: String,
    pub occurrence_summary: String,
    pub source: SourceSpan,
    pub rule_id: String,
    pub context: Vec<(String, AgentContextValue)>,
    pub affected: Vec<String>,
    pub recommended_next_step: RepairStep,
    pub alternatives: Vec<RepairStep>,
}

pub fn json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character <= '\u{001f}' => {
                escaped.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => escaped.push(character),
        }
    }
    escaped.push('"');
    escaped
}

impl fmt::Display for CompilerDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            formatter,
            "{}[{}]: {}",
            self.severity, self.code, self.message
        )?;

        if let Some(primary) = &self.primary {
            writeln!(
                formatter,
                "  at {}:{}..{}",
                primary.source, primary.start, primary.end
            )?;
        }

        for note in &self.notes {
            writeln!(formatter, "  note: {note}")?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        catalogue_definition, catalogue_manifest_json, json_string, DecisionOwner, Diagnostic,
        OperationalLogEvent, OperationalValue, PublicFailureResponse, PublicFailureValue,
        RepairKind, SafeIdentifier, Secret, SourceSpan, CATALOGUE_CODES,
    };
    use serde_json::Value;
    use std::collections::BTreeSet;

    #[test]
    fn emits_stable_json_with_explicit_nullable_location_fields() {
        let mut diagnostic = Diagnostic::error("TEST_CODE").with_note("fix\\path");
        diagnostic.primary = Some(SourceSpan {
            source: "example.jadpo".to_owned(),
            start: 4,
            end: 9,
        });
        diagnostic.related.push(SourceSpan {
            source: "other.jadpo".to_owned(),
            start: 1,
            end: 3,
        });

        let json = diagnostic.to_json();
        assert!(json.contains("\"schemaVersion\":2"));
        assert!(json.contains("\"ruleId\":\"test.code\""));
        assert!(json.contains("\"summary\":\"Code\""));
        assert!(json.contains("\"location\":{\"source\":\"example.jadpo\""));
        assert!(json.contains("\"related\":[{\"source\":\"other.jadpo\""));
        assert!(Diagnostic::warning("TEST_WARNING")
            .to_json()
            .contains("\"location\":null"));
    }

    #[test]
    fn audience_schemas_do_not_share_secret_or_internal_fields() {
        let secret = Secret::new("credential-canary");
        assert_eq!(secret.expose_to_boundary(), &"credential-canary");

        let public = PublicFailureResponse {
            code: "not_found".to_owned(),
            message: "Not found.".to_owned(),
            request_id: "req_1".to_owned(),
            details: vec![(
                "resource".to_owned(),
                PublicFailureValue::Text("public-id".to_owned()),
            )],
        };
        let event = OperationalLogEvent {
            event_name: "operation.failed".to_owned(),
            classification: "unavailable".to_owned(),
            request_id: "req_1".to_owned(),
            trace_id: Some("trace_1".to_owned()),
            semantic_operation_id: "route:items:get".to_owned(),
            source_revision: "src_1".to_owned(),
            safe_attributes: vec![("attempt".to_owned(), OperationalValue::Integer(1))],
        };
        assert!(SafeIdentifier::new("request\ncredential-canary").is_none());
        let public_json = public.to_json();
        assert!(public_json.contains("\"error\":{\"code\":\"not_found\""));
        assert!(public_json.contains("\"request_id\":\"req_1\""));
        for json in [public_json, event.to_json()] {
            assert!(!json.contains("credential-canary"));
            assert!(!json.contains("stack"));
            assert!(!json.contains("provider"));
            assert!(!json.contains("sql"));
        }
    }

    #[test]
    fn catalogue_distinguishes_automatic_guided_and_human_repairs() {
        let automatic = catalogue_definition("FAIL_ATTEMPT_REQUIRED");
        assert_eq!(automatic.repair_kind, RepairKind::AutomaticFix);
        assert_eq!(automatic.decision_owner, DecisionOwner::Compiler);

        let guided = catalogue_definition("ROUTE_PATH_BINDING_MISSING");
        assert_eq!(guided.repair_kind, RepairKind::GuidedChoice);
        assert_eq!(guided.decision_owner, DecisionOwner::Agent);

        let human = catalogue_definition("JADPO_TARGET_AUTH_NOT_IMPLEMENTED");
        assert_eq!(human.repair_kind, RepairKind::HumanDecision);
        assert_eq!(human.decision_owner, DecisionOwner::Human);

        let diagnostic = Diagnostic::error("ROUTE_PATH_BINDING_MISSING");
        assert_eq!(diagnostic.alternatives.len(), 1);
        assert!(diagnostic.to_json().contains("\"alternatives\":[{"));
    }

    #[test]
    fn generated_catalogue_contains_every_representative_compiler_family() {
        let manifest = catalogue_manifest_json();
        for alias in [
            "SYN_UNEXPECTED_TOKEN",
            "TYPE_UNKNOWN_NAME",
            "FAIL_ATTEMPT_REQUIRED",
            "ROUTE_PATH_BINDING_MISSING",
            "CLI_INCIDENT_INVALID",
            "RUNTIME_UNHANDLED_FAULT",
        ] {
            assert!(manifest.contains(alias), "missing {alias}");
        }
        assert!(manifest.contains("\"schemaVersion\":1"));
        assert!(manifest.contains("\"ruleId\":"));
        assert!(manifest.contains("\"summary\":"));
        assert!(
            manifest.contains("tests/compile/fail/60_fallible_call_requires_attempt.expect.json")
        );
        assert!(manifest.contains("\"legacyAliases\":["));
    }

    #[test]
    fn every_catalogue_entry_is_renderable() {
        for code in CATALOGUE_CODES {
            let definition = catalogue_definition(code);
            assert!(!definition.summary.is_empty(), "{code}");
            assert!(!definition.reason.is_empty(), "{code}");
            assert!(!definition.recommended_title.is_empty(), "{code}");
            assert!(!definition.fixtures.is_empty(), "{code}");

            let json = Diagnostic::error(code).to_json();
            assert!(json.contains("\"schemaVersion\":2"), "{code}");
            assert!(
                json.contains(&format!("\"ruleId\":{}", json_string(&definition.rule_id))),
                "{code}"
            );
            assert!(json.contains("\"recommendedNextStep\":"), "{code}");
        }
    }

    #[test]
    fn every_agent_diagnostic_obeys_the_complete_v2_schema() {
        let expected_top_level = BTreeSet::from([
            "alternatives",
            "context",
            "decisionOwner",
            "diagnosticId",
            "helpId",
            "impact",
            "legacyAliases",
            "location",
            "reason",
            "recommendedNextStep",
            "ruleId",
            "schemaVersion",
            "severity",
            "sourceRevision",
            "summary",
        ]);
        let expected_repair = BTreeSet::from([
            "decisionOwner",
            "edits",
            "kind",
            "preferred",
            "preview",
            "reason",
            "title",
        ]);
        let expected_preview = BTreeSet::from(["behavioral", "publicContract"]);
        let expected_impact =
            BTreeSet::from(["affected", "behavioral", "publicContract", "queryId"]);

        for code in CATALOGUE_CODES {
            let mut diagnostic = Diagnostic::error(code).with_source_revision("src_conformance");
            diagnostic.primary = Some(SourceSpan {
                source: "conformance.jadpo".to_owned(),
                start: 2,
                end: 5,
            });
            let value: Value = serde_json::from_str(&diagnostic.to_json())
                .unwrap_or_else(|error| panic!("{code}: invalid JSON: {error}"));
            assert_eq!(value["schemaVersion"], 2, "{code}");
            assert_eq!(object_keys(&value), expected_top_level, "{code}");
            assert_nonempty_string(&value, "summary", code);
            assert_nonempty_string(&value, "reason", code);
            assert_nonempty_string(&value, "ruleId", code);
            assert_nonempty_string(&value, "helpId", code);
            assert_eq!(value["sourceRevision"], "src_conformance", "{code}");
            assert_eq!(value["legacyAliases"], serde_json::json!([code]), "{code}");
            assert_eq!(value["location"]["range"]["start"], 2, "{code}");
            assert_eq!(value["location"]["range"]["end"], 5, "{code}");
            assert!(value["location"]["related"].as_array().is_some(), "{code}");
            assert!(
                value["context"]
                    .as_object()
                    .is_some_and(|items| items.len() <= 16),
                "{code}"
            );
            assert!(
                value["alternatives"]
                    .as_array()
                    .is_some_and(|items| items.len() <= 16),
                "{code}"
            );
            assert_eq!(object_keys(&value["impact"]), expected_impact, "{code}");
            assert!(
                value["impact"]["affected"]
                    .as_array()
                    .is_some_and(|items| items.len() <= 16),
                "{code}"
            );

            let mut repairs = vec![&value["recommendedNextStep"]];
            repairs.extend(value["alternatives"].as_array().unwrap().iter());
            for repair in repairs {
                assert_eq!(object_keys(repair), expected_repair, "{code}");
                assert_eq!(object_keys(&repair["preview"]), expected_preview, "{code}");
                assert_nonempty_string(repair, "title", code);
                assert_nonempty_string(repair, "reason", code);
                assert!(repair["edits"].as_array().is_some(), "{code}");
                assert!(repair["preferred"].is_boolean(), "{code}");
            }

            let packet = diagnostic.to_json();
            for forbidden in [
                "\u{1b}[",
                "credential-canary",
                "provider-canary",
                "stack-canary",
                "sql-canary",
            ] {
                assert!(!packet.contains(forbidden), "{code}: leaked {forbidden}");
            }
        }
    }

    #[test]
    fn public_copy_debt_is_explicit_and_authored_copy_is_human_readable() {
        let mut placeholders = Vec::new();
        for code in CATALOGUE_CODES {
            let definition = catalogue_definition(code);
            if definition.authored_copy {
                for forbidden in [
                    "compiler-enforced",
                    "Update the source to satisfy this rule",
                    "Unexpected token",
                ] {
                    assert!(
                        !format!(
                            "{} {} {}",
                            definition.summary, definition.reason, definition.recommended_title
                        )
                        .contains(forbidden),
                        "{code}: authored copy contains forbidden fallback `{forbidden}`"
                    );
                }
            } else {
                placeholders.push(*code);
                assert!(
                    definition.reason.contains("compiler-enforced")
                        || definition.recommended_title == "Update the source to satisfy this rule",
                    "{code}: unclassified catalogue copy"
                );
            }
        }
        assert!(
            !placeholders.is_empty(),
            "remove the strict ignored gate and this debt assertion when the catalogue is complete"
        );
    }

    #[test]
    fn every_syntax_diagnostic_has_rule_specific_public_copy() {
        for code in CATALOGUE_CODES
            .iter()
            .filter(|code| code.starts_with("SYN_"))
        {
            let definition = catalogue_definition(code);
            assert!(definition.authored_copy, "{code}");
            assert!(!definition.summary.contains("invariant"), "{code}");
            assert!(!definition.reason.contains(code), "{code}");
            assert_ne!(
                definition.recommended_title, "Update the source to satisfy this rule",
                "{code}"
            );
        }
    }

    #[test]
    fn every_semantic_name_diagnostic_has_rule_specific_public_copy() {
        for code in CATALOGUE_CODES
            .iter()
            .filter(|code| code.starts_with("SEM_"))
        {
            let definition = catalogue_definition(code);
            assert!(definition.authored_copy, "{code}");
            assert!(!definition.reason.contains(code), "{code}");
            assert_ne!(
                definition.recommended_title, "Update the source to satisfy this rule",
                "{code}"
            );
        }
    }

    #[test]
    fn every_effect_and_failure_diagnostic_has_rule_specific_public_copy() {
        for code in CATALOGUE_CODES
            .iter()
            .filter(|code| code.starts_with("EFFECT_") || code.starts_with("FAIL_"))
        {
            let definition = catalogue_definition(code);
            assert!(definition.authored_copy, "{code}");
            assert!(!definition.reason.contains(code), "{code}");
            assert_ne!(
                definition.recommended_title, "Update the source to satisfy this rule",
                "{code}"
            );
        }
    }

    #[test]
    fn every_route_diagnostic_has_rule_specific_public_copy() {
        for code in CATALOGUE_CODES
            .iter()
            .filter(|code| code.starts_with("ROUTE_"))
        {
            let definition = catalogue_definition(code);
            assert!(definition.authored_copy, "{code}");
            assert!(!definition.reason.contains(code), "{code}");
            assert_ne!(
                definition.recommended_title, "Update the source to satisfy this rule",
                "{code}"
            );
        }
    }

    #[test]
    fn every_data_model_diagnostic_has_rule_specific_public_copy() {
        for code in CATALOGUE_CODES
            .iter()
            .filter(|code| code.starts_with("DATA_"))
        {
            let definition = catalogue_definition(code);
            assert!(definition.authored_copy, "{code}");
            assert!(!definition.reason.contains(code), "{code}");
            assert_ne!(
                definition.recommended_title, "Update the source to satisfy this rule",
                "{code}"
            );
        }
    }

    #[test]
    fn every_core_type_diagnostic_has_rule_specific_public_copy() {
        const CORE_TYPE_CODES: &[&str] = &[
            "TYPE_ARGUMENT_COUNT",
            "TYPE_ARITHMETIC_OPERAND",
            "TYPE_ASSIGN_IMMUTABLE",
            "TYPE_ASSIGN_UNKNOWN",
            "TYPE_CONSTRUCTOR_ARGUMENT_COUNT",
            "TYPE_CONSTRUCTOR_INPUT",
            "TYPE_DUPLICATE_VARIANT_FIELD",
            "TYPE_FIELD_ON_NON_RECORD",
            "TYPE_INCOMPARABLE",
            "TYPE_INVALID_LITERAL",
            "TYPE_LOGICAL_OPERAND",
            "TYPE_MISMATCH",
            "TYPE_MISSING_FIELD",
            "TYPE_MISSING_VARIANT_FIELD",
            "TYPE_NOT_RECORD",
            "TYPE_NULLABLE_SELECTION",
            "TYPE_ORDERING_OPERAND",
            "TYPE_PRIMITIVE_SIGNATURE",
            "TYPE_SIBLING_MISMATCH",
            "TYPE_UNARY_OPERAND",
            "TYPE_UNKNOWN_ENUM_VARIANT",
            "TYPE_UNKNOWN_FIELD",
            "TYPE_UNKNOWN_NAME",
            "TYPE_UNKNOWN_VALUE",
            "TYPE_UNKNOWN_VARIANT_FIELD",
        ];

        for code in CORE_TYPE_CODES {
            assert!(CATALOGUE_CODES.contains(code), "{code}");
            let definition = catalogue_definition(code);
            assert!(definition.authored_copy, "{code}");
            assert!(!definition.reason.contains(code), "{code}");
            assert_ne!(
                definition.recommended_title, "Update the source to satisfy this rule",
                "{code}"
            );
        }
    }

    #[test]
    fn every_match_type_diagnostic_has_rule_specific_public_copy() {
        const MATCH_TYPE_CODES: &[&str] = &[
            "TYPE_MATCH_DUPLICATE_BINDING",
            "TYPE_MATCH_DUPLICATE_PATTERN",
            "TYPE_MATCH_NON_EXHAUSTIVE",
            "TYPE_MATCH_PATTERN_TYPE",
            "TYPE_MATCH_SOME_NON_OPTIONAL",
            "TYPE_MATCH_UNKNOWN_BINDING",
            "TYPE_MATCH_UNKNOWN_VARIANT",
            "TYPE_MATCH_UNREACHABLE_PATTERN",
            "TYPE_MATCH_VARIANT_BINDINGS_REQUIRED",
            "TYPE_MATCH_WILDCARD_REQUIRED",
        ];

        for code in MATCH_TYPE_CODES {
            assert!(CATALOGUE_CODES.contains(code), "{code}");
            let definition = catalogue_definition(code);
            assert!(definition.authored_copy, "{code}");
            assert!(!definition.reason.contains(code), "{code}");
            assert_ne!(
                definition.recommended_title, "Update the source to satisfy this rule",
                "{code}"
            );
        }
    }

    #[test]
    fn every_type_diagnostic_has_rule_specific_public_copy() {
        for code in CATALOGUE_CODES
            .iter()
            .filter(|code| code.starts_with("TYPE_"))
        {
            let definition = catalogue_definition(code);
            assert!(definition.authored_copy, "{code}");
            assert!(!definition.reason.contains(code), "{code}");
            assert_ne!(
                definition.recommended_title, "Update the source to satisfy this rule",
                "{code}"
            );
        }
    }

    #[test]
    fn every_module_format_and_lsp_diagnostic_has_rule_specific_public_copy() {
        for code in CATALOGUE_CODES.iter().filter(|code| {
            code.starts_with("MOD_") || code.starts_with("FMT_") || code.starts_with("LSP_")
        }) {
            let definition = catalogue_definition(code);
            assert!(definition.authored_copy, "{code}");
            assert!(!definition.reason.contains(code), "{code}");
            assert_ne!(
                definition.recommended_title, "Update the source to satisfy this rule",
                "{code}"
            );
        }
    }

    #[test]
    fn every_index_and_toolchain_diagnostic_has_rule_specific_public_copy() {
        for code in CATALOGUE_CODES
            .iter()
            .filter(|code| code.starts_with("INDEX_") || code.starts_with("JADPO_"))
        {
            let definition = catalogue_definition(code);
            assert!(definition.authored_copy, "{code}");
            assert!(!definition.reason.contains(code), "{code}");
            assert_ne!(
                definition.recommended_title, "Update the source to satisfy this rule",
                "{code}"
            );
        }
    }

    #[test]
    #[ignore = "DX2 copy debt: this becomes the permanent non-ignored gate when all families are authored"]
    fn strict_public_catalogue_has_no_placeholders_and_every_code_has_a_real_fixture() {
        let failures = CATALOGUE_CODES
            .iter()
            .filter_map(|code| {
                let definition = catalogue_definition(code);
                let has_real_fixture = definition.fixtures.iter().any(|fixture| {
                    *fixture
                        != "jadpo/crates/diagnostics/src/lib.rs#every_catalogue_entry_is_renderable"
                });
                (!definition.authored_copy || !has_real_fixture).then_some(format!(
                    "{code}: copy={}, fixture={}",
                    if definition.authored_copy {
                        "authored"
                    } else {
                        "placeholder"
                    },
                    if has_real_fixture {
                        "present"
                    } else {
                        "missing"
                    }
                ))
            })
            .collect::<Vec<_>>();
        assert!(failures.is_empty(), "\n{}", failures.join("\n"));
    }

    fn object_keys(value: &Value) -> BTreeSet<&str> {
        value
            .as_object()
            .expect("expected object")
            .keys()
            .map(String::as_str)
            .collect()
    }

    fn assert_nonempty_string(value: &Value, key: &str, code: &str) {
        assert!(
            value[key]
                .as_str()
                .is_some_and(|text| !text.trim().is_empty()),
            "{code}: `{key}` must be a non-empty string"
        );
    }
}
