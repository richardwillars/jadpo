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
    let syntax_copy = syntax_catalogue_copy(code);
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
                _ => syntax_copy
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
        _ => syntax_copy
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
        _ => syntax_copy.map_or_else(
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
    let authored_copy = syntax_copy.is_some()
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
