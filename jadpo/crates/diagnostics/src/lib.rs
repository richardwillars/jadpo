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
    Name(String),
    ActualKind(String),
    Examples(String),
    SuggestedName(String),
    Usage(String),
    Received(String),
    Subject(String),
    Field(String),
    ExpectedCount(String),
    ReceivedCount(String),
    Constraint(String),
    Failure(String),
    OtherFailure(String),
    CodeValue(String),
    Scope(String),
    Operation(String),
    Failures(String),
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
            Self::Name(value) => ("name", value),
            Self::ActualKind(value) => ("actualKind", value),
            Self::Examples(value) => ("examples", value),
            Self::SuggestedName(value) => ("suggestedName", value),
            Self::Usage(value) => ("usage", value),
            Self::Received(value) => ("received", value),
            Self::Subject(value) => ("subject", value),
            Self::Field(value) => ("field", value),
            Self::ExpectedCount(value) => ("expectedCount", value),
            Self::ReceivedCount(value) => ("receivedCount", value),
            Self::Constraint(value) => ("constraint", value),
            Self::Failure(value) => ("failure", value),
            Self::OtherFailure(value) => ("otherFailure", value),
            Self::CodeValue(value) => ("codeValue", value),
            Self::Scope(value) => ("scope", value),
            Self::Operation(value) => ("operation", value),
            Self::Failures(value) => ("failures", value),
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

pub fn diagnostic_help_url(help_id: &str) -> String {
    format!("https://jadpo.dev/docs/{help_id}")
}

impl CompilerDiagnostic {
    pub fn error(code: &'static str) -> Self {
        Self::new(code, Severity::Error)
    }

    fn new(code: &'static str, severity: Severity) -> Self {
        let definition = catalogue_definition(code);
        let (behavioral_impact, public_contract_impact) = match severity {
            Severity::Error => (
                "Compilation is blocked until this problem is fixed.",
                "No public-contract change has been applied.",
            ),
            Severity::Warning => (
                "The project can still build; this recommendation is advisory.",
                "No public-contract change has been applied.",
            ),
            Severity::Note => (
                "This information does not block compilation.",
                "No public-contract change has been applied.",
            ),
        };
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
                behavioral: behavioral_impact.to_owned(),
                public_contract: public_contract_impact.to_owned(),
                affected: Vec::new(),
                query_id: None,
            }),
            primary: None,
            related: Vec::new(),
            notes: Vec::new(),
        };
        if code == "ROUTE_AUTH_VALUE_INVALID" {
            "Compilation is blocked until the route authentication boundary is chosen."
                .clone_into(&mut diagnostic.impact.behavioral);
            "The public route security boundary is unresolved; the compiler will not guess."
                .clone_into(&mut diagnostic.impact.public_contract);
            "No authentication behavior changes until a human chooses a valid boundary."
                .clone_into(&mut diagnostic.recommended_next_step.behavioral_effect);
            "The route's public security contract remains unchanged while compilation is blocked."
                .clone_into(&mut diagnostic.recommended_next_step.public_contract_effect);
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
        self.recommended_next_step.reason.clone_from(&self.reason);
        self.refresh_semantic_name_copy();
        self.refresh_type_copy();
        self.refresh_failure_copy();
    }

    fn refresh_semantic_name_copy(&mut self) {
        let value = |key: &str| {
            self.context
                .iter()
                .find_map(|(candidate, value)| (candidate == key).then_some(value.as_str()))
        };
        let Some(name) = value("name") else {
            return;
        };
        let expected = value("expected").unwrap_or("declaration");
        let suggestion = value("suggestedName");
        let examples = value("examples");
        let usage = value("usage");

        match self.code {
            "SEM_DUPLICATE_DECLARATION" => {
                self.message = format!("`{name}` is already defined");
                self.reason = format!(
                    "There is already a visible declaration named `{name}`, so references would not know which one to use."
                );
                "Rename this declaration or remove the duplicate"
                    .clone_into(&mut self.recommended_next_step.title);
            }
            "SEM_UNKNOWN_NAME" => {
                self.message = format!("`{name}` isn't defined");
                self.reason = semantic_expected_reason(expected, examples, suggestion, usage);
                self.recommended_next_step.title =
                    semantic_unknown_name_guidance(name, expected, suggestion);
            }
            "SEM_WRONG_NAME_KIND" => {
                self.message = format!("`{name}` can't be used here");
                self.reason = semantic_expected_reason(expected, examples, None, usage);
                self.recommended_next_step.title = semantic_replacement_guidance(expected);
            }
            "SEM_UNKNOWN_CALLEE" => {
                self.message = format!("`{name}` isn't defined");
                self.reason =
                    semantic_expected_reason("function or action", None, suggestion, usage);
                self.recommended_next_step.title = suggestion.map_or_else(
                    || format!("Define `{name}` as a function or action, or import it"),
                    |suggested| {
                        format!(
                            "Use `{suggested}`, define `{name}` as a function or action, or import it"
                        )
                    },
                );
            }
            "SEM_NOT_CALLABLE" => {
                self.message = format!("`{name}` can't be called");
                "Only a function or action can be followed by parentheses and called."
                    .clone_into(&mut self.reason);
                "Call a function or action instead, or remove the parentheses"
                    .clone_into(&mut self.recommended_next_step.title);
            }
            _ => return,
        }
        self.recommended_next_step.reason.clone_from(&self.reason);
    }

    fn refresh_type_copy(&mut self) {
        let value = |key: &str| {
            self.context
                .iter()
                .find_map(|(candidate, value)| (candidate == key).then_some(value.as_str()))
        };
        let name = value("name");
        let subject = value("subject");
        let field = value("field");
        let expected = value("expected");
        let received = value("received");
        let suggestion = value("suggestedName");

        match self.code {
            "TYPE_MISMATCH" | "TYPE_SIBLING_MISMATCH" => {
                let (Some(expected), Some(received)) = (expected, received) else {
                    return;
                };
                self.message = format!("`{received}` can't be used where `{expected}` is required");
                self.reason = if self.code == "TYPE_SIBLING_MISMATCH" {
                    format!(
                        "`{received}` and `{expected}` are separate named types, even if they build on the same base type."
                    )
                } else {
                    format!("This value has type `{received}`, but this part of the source needs `{expected}`.")
                };
                self.recommended_next_step.title = format!("Use a `{expected}` value here");
            }
            "TYPE_ARGUMENT_COUNT" | "TYPE_CONSTRUCTOR_ARGUMENT_COUNT" => {
                let (Some(expected_count), Some(received_count)) =
                    (value("expectedCount"), value("receivedCount"))
                else {
                    return;
                };
                let subject = name.unwrap_or("This call");
                self.message = format!(
                    "`{subject}` expects {expected_count} {}, but received {received_count}",
                    plural("argument", expected_count)
                );
                "Each declared parameter needs one value in the same order."
                    .clone_into(&mut self.reason);
                self.recommended_next_step.title = format!(
                    "Pass {expected_count} {} to `{subject}`",
                    plural("argument", expected_count)
                );
            }
            "TYPE_CONSTRUCTOR_INPUT" => {
                let (Some(expected), Some(received)) = (expected, received) else {
                    return;
                };
                let constructor = name.unwrap_or("This type");
                self.message = format!("`{constructor}` can't be constructed from `{received}`");
                self.reason = format!("`{constructor}` builds on `{expected}`, so its value must be compatible with `{expected}`.");
                self.recommended_next_step.title =
                    format!("Pass a `{expected}` value to `{constructor}`");
            }
            "TYPE_MISSING_FIELD" | "TYPE_MISSING_VARIANT_FIELD" => {
                let (Some(subject), Some(field)) = (subject, field) else {
                    return;
                };
                self.message = format!("`{subject}` is missing the required `{field}` field");
                self.reason = format!("`{field}` is required by the `{subject}` declaration.");
                self.recommended_next_step.title = format!("Add `{field}: ...` to `{subject}`");
            }
            "TYPE_UNKNOWN_FIELD" | "TYPE_UNKNOWN_VARIANT_FIELD" => {
                let (Some(subject), Some(field)) = (subject, field) else {
                    return;
                };
                self.message = format!("`{subject}` has no `{field}` field");
                self.reason = suggestion.map_or_else(
                    || format!("`{field}` is not declared on `{subject}`."),
                    |suggested| {
                        format!(
                            "`{field}` is not declared on `{subject}`. Did you mean `{suggested}`?"
                        )
                    },
                );
                self.recommended_next_step.title = suggestion.map_or_else(
                    || format!("Use a field declared on `{subject}`"),
                    |suggested| format!("Use `{suggested}` instead of `{field}`"),
                );
            }
            "TYPE_UNKNOWN_VALUE" => {
                let Some(name) = name else { return };
                self.message = format!("`{name}` isn't defined in this scope");
                self.reason = suggestion.map_or_else(
                    || "Jadpo could not find a parameter or local value with this name.".to_owned(),
                    |suggested| {
                        format!("Jadpo could not find this name. Did you mean `{suggested}`?")
                    },
                );
                self.recommended_next_step.title = suggestion.map_or_else(
                    || format!("Define `{name}` before using it"),
                    |suggested| format!("Use `{suggested}` instead of `{name}`"),
                );
            }
            "TYPE_UNKNOWN_ENUM_VARIANT" => {
                let (Some(subject), Some(name)) = (subject, name) else {
                    return;
                };
                self.message = format!("`{subject}` has no `{name}` variant");
                self.reason = suggestion.map_or_else(
                    || format!("`{name}` is not one of the variants declared by `{subject}`."),
                    |suggested| {
                        format!(
                            "`{name}` is not declared by `{subject}`. Did you mean `{suggested}`?"
                        )
                    },
                );
                self.recommended_next_step.title = suggestion.map_or_else(
                    || format!("Use a variant declared by `{subject}`"),
                    |suggested| format!("Use `{subject}.{suggested}`"),
                );
            }
            "TYPE_FIELD_ON_NON_RECORD" => {
                let (Some(received), Some(field)) = (received, field) else {
                    return;
                };
                self.message = format!("`{received}` values have no `{field}` field");
                self.reason = format!("`{received}` is not a record with named fields.");
                "Use a record value before `.`, or remove the field access"
                    .clone_into(&mut self.recommended_next_step.title);
            }
            "TYPE_NULLABLE_SELECTION" => {
                let (Some(received), Some(field)) = (received, field) else {
                    return;
                };
                self.message =
                    format!("`{received}` may be `none`, so `{field}` can't be read yet");
                "Jadpo needs the `some(...)` case before it can safely read this field."
                    .clone_into(&mut self.reason);
                "Match the nullable value and read the field inside `some(...)`"
                    .clone_into(&mut self.recommended_next_step.title);
            }
            "TYPE_PRIMITIVE_SIGNATURE" => {
                let (Some(received), Some(usage)) = (received, value("usage")) else {
                    return;
                };
                self.message = format!("This {usage} uses `{received}` directly");
                self.reason = format!("A {usage} needs a named Jadpo type so its meaning and validation rules are explicit.");
                self.recommended_next_step.title =
                    format!("Define a named type based on `{received}` and use it here");
            }
            "TYPE_INVALID_LITERAL" => {
                let Some(subject) = subject else { return };
                self.message = format!("This value doesn't meet `{subject}`'s rules");
                if let Some(constraint) = value("constraint") {
                    self.reason = format!(
                        "The value does not satisfy `{subject}`'s `{constraint}` constraint."
                    );
                    self.recommended_next_step.title =
                        format!("Change the value so it satisfies `{constraint}`");
                }
            }
            _ => return,
        }
        self.recommended_next_step.reason.clone_from(&self.reason);
    }

    fn refresh_failure_copy(&mut self) {
        let value = |key: &str| {
            self.context
                .iter()
                .find_map(|(candidate, value)| (candidate == key).then_some(value.as_str()))
        };
        let callable = value("callable");
        let failure = value("failure");
        let field = value("field");

        match self.code {
            "EFFECT_FUNCTION_CALLS_ACTION" => {
                let (Some(function), Some(action)) = (callable, value("name")) else {
                    return;
                };
                self.message = format!("Function `{function}` can't call action `{action}`");
                self.reason = format!("`{function}` is a function, so it cannot run persistence or other action effects through `{action}`.");
                self.recommended_next_step.title = format!(
                    "Move this call into an action, or call a function instead of `{action}`"
                );
            }
            "EFFECT_FUNCTION_PERSISTENCE" => {
                let Some(function) = callable else { return };
                self.message = format!("Function `{function}` can't use persistence");
                "Create, query, update, and delete operations are available only inside actions."
                    .clone_into(&mut self.reason);
                "Move this persistence operation into an action"
                    .clone_into(&mut self.recommended_next_step.title);
            }
            "FAIL_ATTEMPT_REQUIRED" => {
                let usage = value("usage").unwrap_or("operation");
                self.message = format!("This {usage} can fail and requires `attempt`");
                "`attempt` makes it clear that a declared failure may leave the current function or action."
                    .clone_into(&mut self.reason);
                self.recommended_next_step.title = format!("Add `attempt` before this {usage}");
            }
            "FAIL_CONTEXT_FIELD_OVERLAP" => {
                let (Some(failure), Some(field)) = (failure, field) else {
                    return;
                };
                self.message = format!("`{failure}.{field}` is both public and internal");
                "The same field cannot be returned to callers and hidden from them at the same time."
                    .clone_into(&mut self.reason);
                self.recommended_next_step.title =
                    format!("Keep `{field}` in either `public` or `internal`, not both");
            }
            "FAIL_DUPLICATE_CODE" => {
                let (Some(code), Some(first), Some(second)) =
                    (value("codeValue"), value("otherFailure"), failure)
                else {
                    return;
                };
                self.message = format!("Failure code `{code}` is already used by `{first}`");
                self.reason = format!("`{second}` and `{first}` cannot share a public code because callers use it to identify one failure.");
                self.recommended_next_step.title =
                    format!("Give `{second}` a different stable public code");
            }
            "FAIL_DUPLICATE_CONTEXT_FIELD" => {
                let (Some(failure), Some(field)) = (failure, field) else {
                    return;
                };
                self.message = format!("`{field}` is supplied more than once for `{failure}`");
                self.reason =
                    format!("A rejected `{failure}` value can contain one `{field}` value.");
                self.recommended_next_step.title =
                    format!("Keep one `{field}: ...` entry in this rejection");
            }
            "FAIL_DUPLICATE_DECLARATION" => {
                let (Some(callable), Some(failure)) = (callable, failure) else {
                    return;
                };
                self.message = format!("`{failure}` is listed more than once after `fails`");
                self.reason =
                    format!("Listing `{failure}` twice does not change what `{callable}` can do.");
                self.recommended_next_step.title =
                    format!("Keep one `{failure}` entry after `fails`");
            }
            "FAIL_MISSING_CONTEXT_FIELD" => {
                let (Some(failure), Some(field)) = (failure, field) else {
                    return;
                };
                let scope = value("scope").unwrap_or("required");
                self.message = format!("`{failure}` is missing the {scope} `{field}` field");
                self.reason = format!("`{field}` is required whenever `{failure}` is rejected.");
                self.recommended_next_step.title =
                    format!("Add `{field}: ...` to this `{failure}` rejection");
            }
            "FAIL_MUTATION_CONFLICT_NOT_CONFLICT"
            | "FAIL_PATCH_EMPTY_NOT_INVALID_VALUE"
            | "FAIL_REQUIRED_MUTATION_NOT_NOT_FOUND"
            | "FAIL_REQUIRED_QUERY_NOT_NOT_FOUND" => {
                let (Some(failure), Some(expected), Some(received)) =
                    (failure, value("expected"), value("received"))
                else {
                    return;
                };
                let usage = value("usage").unwrap_or("this failure binding");
                self.message =
                    format!("`{failure}` uses `{received}`, but {usage} requires `{expected}`");
                self.reason =
                    format!("`{failure}` must use the predefined `{expected}` category here.");
                self.recommended_next_step.title =
                    format!("Use a failure declared with `kind {expected}`");
            }
            "FAIL_STALE_DECLARATION" => {
                let (Some(callable), Some(failures)) = (callable, value("failures")) else {
                    return;
                };
                self.message = format!("`{failures}` can never leave `{callable}`");
                self.reason = format!("`{callable}` lists this after `fails`, but no call or `reject` can produce it.");
                self.recommended_next_step.title =
                    format!("Remove `{failures}` from `{callable}`'s `fails` list");
            }
            "FAIL_UNDECLARED_PROPAGATION" => {
                let (Some(callable), Some(failure)) = (callable, failure) else {
                    return;
                };
                self.message =
                    format!("`{failure}` can leave `{callable}` but isn't listed after `fails`");
                self.reason =
                    format!("Callers of `{callable}` need to know that `{failure}` can happen.");
                self.recommended_next_step.title =
                    format!("Add `{failure}` after `fails`, or handle it inside `{callable}`");
            }
            "FAIL_UNKNOWN_CONTEXT_FIELD" => {
                let (Some(failure), Some(field)) = (failure, field) else {
                    return;
                };
                self.message = format!("`{failure}` has no `{field}` field");
                self.reason = format!(
                    "`{field}` is not declared in the public or internal fields of `{failure}`."
                );
                self.recommended_next_step.title =
                    format!("Remove `{field}`, or declare it on `{failure}`");
            }
            _ => return,
        }
        self.recommended_next_step.reason.clone_from(&self.reason);
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
            "Jadpo needs a value here, such as a literal, name, function call, constructed value, match, or persistence operation.",
            "Add the value or operation you intended",
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
            "Inverse relationship requires `optional` or `many`",
            "Jadpo needs to know whether this relationship can return at most one record or a list of records.",
            "Add `optional` or `many` to the inverse relationship",
        ),
        "SYN_EXPECTED_INVOCATION" => (
            "Route `run:` requires an action invocation",
            "Named route behavior calls an action with parentheses. A bare name or another expression does not define the route's argument mapping.",
            "Call the action using `name(arguments)` after `run:`",
        ),
        "SYN_EXPECTED_LITERAL" => (
            "Expected a literal value",
            "Jadpo needs a value written directly here, such as text, a number, `true`, `false`, or `none`.",
            "Replace this with the required literal value",
        ),
        "SYN_EXPECTED_NAME" => (
            "Expected a name",
            "Jadpo needs the name of a declaration, field, local value, or qualified path here.",
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
            "Failure requires a predefined category",
            "Every failure needs a `kind`, such as `InvalidValue`, `NotFound`, `Conflict`, or `Unavailable`, so Jadpo knows how to present it at a route boundary.",
            "Add a predefined category with `kind <Category>`",
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
        "SYN_INVALID_ESCAPE" => (
            "String contains an unsupported escape sequence",
            "Jadpo strings support escapes for quotes, backslashes, newline, carriage return, and tab; this backslash sequence has no defined value.",
            "Use a supported string escape or remove the backslash",
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
            "A type definition builds on an existing type, such as `Text`, `Uuid`, or another named type.",
            "Add the type it builds on after `=`",
        ),
        "SYN_UNEXPECTED_TOKEN" => (
            "Expected {expected}",
            "Found `{found}` while parsing this construct. Jadpo requires {expected} at this location, so parsing stops rather than guessing the authored structure.",
            "Provide {expected}",
        ),
        "SYN_UNEXPECTED_CHARACTER" => (
            "Character is not valid Jadpo syntax",
            "This character is not part of any Jadpo name, value, keyword, or punctuation.",
            "Remove the character or replace it with valid Jadpo syntax",
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
            "A name is defined more than once",
            "Every visible name must identify one thing. Two declarations with the same name would make later references ambiguous.",
            "Rename one of the declarations or remove the duplicate",
        ),
        "SEM_UNKNOWN_NAME" => (
            "This name isn't defined",
            "Jadpo could not find this name among local values, declarations, imports, or built-in types.",
            "Correct the name, define it, or import it",
        ),
        "SEM_WRONG_NAME_KIND" => (
            "This name can't be used here",
            "The name exists, but this part of the source needs something different, such as a type, failure, function, or predefined category.",
            "Use a name that is valid here",
        ),
        "SEM_UNKNOWN_CALLEE" => (
            "Called function or action isn't defined",
            "Jadpo could not find a visible function or action with this name.",
            "Correct the name, define the function or action, or import it",
        ),
        "SEM_NOT_CALLABLE" => (
            "This name can't be called",
            "Only a function or action can be followed by parentheses and called.",
            "Call a function or action instead, or remove the parentheses",
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
            "Move the call into an action, or call a function instead",
        ),
        "EFFECT_FUNCTION_PERSISTENCE" => (
            "Function cannot perform persistence",
            "Create, query, update, and delete expressions access stored state and are permitted only inside actions; functions remain pure and deterministic.",
            "Move the persistence expression into an action",
        ),
        "FAIL_ATTEMPT_REQUIRED" => (
            "This operation can fail and requires `attempt`",
            "`attempt` makes it clear that a declared failure may leave the current function or action.",
            "Add `attempt` before this operation",
        ),
        "FAIL_CONTEXT_FIELD_OVERLAP" => (
            "Failure field is both public and internal",
            "The same field cannot be returned to callers and hidden from them at the same time.",
            "Keep the field in either `public` or `internal`, not both",
        ),
        "FAIL_DUPLICATE_CODE" => (
            "Failure code is already in use",
            "Callers use this code to identify a failure, so every public failure code must be unique.",
            "Give one failure a distinct stable public code",
        ),
        "FAIL_DUPLICATE_CONTEXT_FIELD" => (
            "Failure context field is supplied more than once",
            "A rejected failure has one flat context value; assigning a field twice would make its disclosed and internal value ambiguous.",
            "Keep one value for the repeated context field",
        ),
        "FAIL_DUPLICATE_DECLARATION" => (
            "Failure is repeated after `fails`",
            "Listing the same failure twice does not change what can happen and makes the declaration harder to review.",
            "Remove the repeated failure from the `fails` list",
        ),
        "FAIL_MISSING_CONTEXT_FIELD" => (
            "Rejected failure is missing required context",
            "Every field declared by a failure must be supplied when that failure is rejected so its public and internal values are complete and typed.",
            "Add the missing field to the `reject` value",
        ),
        "FAIL_MUTATION_CONFLICT_NOT_CONFLICT" => (
            "`conflict:` must use a `Conflict` failure",
            "A uniqueness or write conflict needs a failure declared with the predefined `Conflict` category.",
            "Bind the conflict to a failure declared with `kind Conflict`",
        ),
        "FAIL_PATCH_EMPTY_NOT_INVALID_VALUE" => (
            "`empty:` must use an `InvalidValue` failure",
            "A patch with no supplied changes is invalid input, so its failure must use the predefined `InvalidValue` category.",
            "Bind `empty:` to a failure declared with `kind InvalidValue`",
        ),
        "FAIL_REQUIRED_MUTATION_NOT_NOT_FOUND" => (
            "`missing:` must use a `NotFound` failure",
            "When a required update or delete finds no entity, its failure must use the predefined `NotFound` category.",
            "Bind `missing:` to a failure declared with `kind NotFound`",
        ),
        "FAIL_REQUIRED_QUERY_NOT_NOT_FOUND" => (
            "`missing:` must use a `NotFound` failure",
            "When a required query finds no entity, its failure must use the predefined `NotFound` category.",
            "Bind `missing:` to a failure declared with `kind NotFound`",
        ),
        "FAIL_STALE_DECLARATION" => (
            "A listed failure can never happen",
            "Every failure after `fails` must be able to leave the function or action through a call or `reject`.",
            "Remove the stale `fails` entry",
        ),
        "FAIL_UNDECLARED_PROPAGATION" => (
            "A possible failure is missing after `fails`",
            "A function or action must list every failure that can leave its body so callers know what to handle.",
            "Add the failure to `fails`, or handle it before it leaves",
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
            "A value inside the URL path is always required and can declare only its type. It cannot be nullable, optional, stored, indexed, or a relationship.",
            "Remove the modifier and keep only `name: Type`",
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
            "A compound identity, uniqueness rule, or index joins multiple fields. A rule for one field belongs directly on that field.",
            "Add the other participating fields, or use a field-level modifier",
        ),
        "DATA_CONSTRAINT_DUPLICATE_FIELD" => (
            "Compound constraint repeats a field",
            "Each field can appear once in a compound constraint. Repeating it does not add anything and makes the intended key unclear.",
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
            "Two persistence constraints over the same fields would create duplicate or conflicting database rules.",
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
            "An entity has one identity used by references, stored records, and generated identifiers.",
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
            "A stored reference must name a field that is actually declared on the target entity.",
            "Correct the referenced target field",
        ),
        "DATA_RELATIONSHIP_TARGET_NOT_KEY" => (
            "Reference target is not an identity or unique key",
            "A relationship must point to a stable identity or unique field set; otherwise one stored reference could resolve to multiple target records.",
            "Reference the target identity or another unique key",
        ),
        "DATA_RELATIONSHIP_TYPE_MISMATCH" => (
            "Reference field type does not match its target",
            "The reference and its target must use the same named type. For example, an `OrderId` cannot point to a `CustomerId` field.",
            "Use the referenced field's named type for this reference",
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
            "Add or remove arguments to match the function or action parameters",
        ),
        "TYPE_ARITHMETIC_OPERAND" => (
            "Arithmetic requires numbers",
            "Operators such as `+`, `-`, `*`, and `/` accept compatible numeric values, not text, Boolean values, records, or unrelated named types.",
            "Use numbers on both sides, or choose an operation supported by these values",
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
            "Type constructor needs exactly one value",
            "A named scalar type, such as `CustomerId(...)`, wraps one compatible value and then checks its constraints.",
            "Pass exactly one value between the parentheses",
        ),
        "TYPE_CONSTRUCTOR_INPUT" => (
            "Type constructor received the wrong type of value",
            "The value between the parentheses must match the type after `=` in this type's definition.",
            "Pass a value that matches the type definition",
        ),
        "TYPE_DUPLICATE_VARIANT_FIELD" => (
            "Enum variant payload field is repeated",
            "A data-carrying enum variant has one exact payload record; duplicate field names would make construction and pattern binding ambiguous.",
            "Keep one declaration of the variant payload field",
        ),
        "TYPE_FIELD_ON_NON_RECORD" => (
            "This value has no fields",
            "Only a value, input, output, entity, failure, or enum payload has named fields that can be selected with `.`.",
            "Use a record value before `.`, or remove the field access",
        ),
        "TYPE_INCOMPARABLE" => (
            "Values cannot be compared for equality",
            "Both sides of `==` or `!=` must have compatible types. Separately named types are not interchangeable just because they share the same base type.",
            "Compare values with the same type",
        ),
        "TYPE_INVALID_LITERAL" => (
            "Value does not meet this type's rules",
            "The written value breaks a constraint on the named type, such as length, range, pattern, or format.",
            "Change the value so it meets the type's constraints",
        ),
        "TYPE_LOGICAL_OPERAND" => (
            "Logical operator requires Boolean operands",
            "`and` and `or` combine Boolean conditions only; Jadpo does not coerce numbers, text, records, or optional values to truthiness.",
            "Use Boolean expressions on both sides of the logical operator",
        ),
        "TYPE_MISMATCH" => (
            "Value has the wrong type",
            "Jadpo keeps separately named types distinct and converts them only where the type definitions explicitly allow it.",
            "Use a value of the required type, or construct that type explicitly",
        ),
        "TYPE_MISSING_FIELD" => (
            "Record construction is missing a required field",
            "A constructed record must provide every required field from its declaration exactly once.",
            "Add the missing required field to the constructor",
        ),
        "TYPE_MISSING_VARIANT_FIELD" => (
            "Enum variant construction is missing a payload field",
            "This enum variant carries data and must provide every required field declared for that variant.",
            "Add the missing field to the variant payload",
        ),
        "TYPE_NOT_RECORD" => (
            "This type cannot be constructed with `{ ... }`",
            "Braces construct a value, input, output, entity, failure, or enum variant. A scalar named type uses parentheses instead.",
            "Use a record with braces, or construct a scalar type with parentheses",
        ),
        "TYPE_NULLABLE_SELECTION" => (
            "Value may be `none`, so its field cannot be read yet",
            "A nullable record may contain no value. Jadpo needs the `some(...)` case before it can safely read a field.",
            "Use a nullable match and select the field inside the `some(...)` arm",
        ),
        "TYPE_ORDERING_OPERAND" => (
            "These values cannot be ordered",
            "`<`, `<=`, `>`, and `>=` require compatible ordered values. Records, Boolean values, lists, and unrelated named types have no defined order.",
            "Compare values of the same ordered type",
        ),
        "TYPE_PRIMITIVE_SIGNATURE" => (
            "Function or action signature uses a built-in type directly",
            "Parameters and return values must use a named Jadpo type so their meaning and validation rules are explicit.",
            "Define a named type and use it in this signature",
        ),
        "TYPE_SIBLING_MISMATCH" => (
            "Value has a different named type",
            "Two named types remain separate even when both build on the same base type. For example, an `OrderId` is not a `CustomerId`.",
            "Use the required named type, or construct it explicitly from an allowed value",
        ),
        "TYPE_UNARY_OPERAND" => (
            "Operator cannot be used with this value",
            "`not` requires a Boolean value, while numeric negation requires a supported number.",
            "Use a Boolean after `not`, or a number after `-`",
        ),
        "TYPE_UNKNOWN_ENUM_VARIANT" => (
            "Enum variant does not exist",
            "The selected variant is not declared by this closed enum, so it cannot be constructed or matched.",
            "Use one of the enum's declared variants",
        ),
        "TYPE_UNKNOWN_FIELD" => (
            "Record field does not exist",
            "This field is not declared on the record. Jadpo does not add fields dynamically at runtime.",
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
            "This field is not declared on the selected enum variant.",
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
            "Match pattern names the same field more than once",
            "Each enum payload field creates one local value inside its match arm. Repeating it would create two local values for the same field.",
            "Keep one local name for each payload field",
        ),
        "TYPE_MATCH_DUPLICATE_PATTERN" => (
            "Match pattern is already covered",
            "An earlier arm already handles this literal, enum variant, optional case, or wildcard, so this arm cannot select a new case.",
            "Remove the duplicate arm or change it to an uncovered case",
        ),
        "TYPE_MATCH_NON_EXHAUSTIVE" => (
            "Match does not cover every possible value",
            "Every enum variant, Boolean value, or nullable case must select a match arm.",
            "Add arms for every missing case",
        ),
        "TYPE_MATCH_PATTERN_TYPE" => (
            "Pattern cannot match this value",
            "This pattern belongs to a different enum or built-in type than the value being matched.",
            "Use a pattern from the value's type",
        ),
        "TYPE_MATCH_SOME_NON_OPTIONAL" => (
            "`some` pattern requires a nullable value",
            "`some(name)` handles the present case of a nullable value, but this value is already guaranteed to be present.",
            "Match the value directly, or make it nullable if absence is intended",
        ),
        "TYPE_MATCH_UNKNOWN_BINDING" => (
            "Pattern names a field this enum variant does not have",
            "The local name in this pattern must match a field declared on the selected enum variant.",
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
            "Enum variant pattern must name its fields",
            "This enum variant carries fields. The pattern must name them so the match arm can use their values.",
            "Add the variant's field names in parentheses",
        ),
        "TYPE_MATCH_WILDCARD_REQUIRED" => (
            "Open value space requires a wildcard arm",
            "This value can have more values than the listed literal patterns cover.",
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
            "`create` stores an entity. Values, inputs, outputs, failures, and scalar types are not stored as entity records.",
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
            "This output requires a parent value, but an optional query may find no parent.",
            "Use a required or many-parent query for this include",
        ),
        "TYPE_INCLUDE_RESULT_MISMATCH" => (
            "Includes use different result output types",
            "All relationships loaded by one query are returned together, so every include must name the same `into` output type.",
            "Use the same result output type for every include",
        ),
        "TYPE_INCLUDE_RESULT_NOT_OUTPUT" => (
            "Include result type is not an output",
            "An include returns an explicitly declared boundary projection. Entities, values, and inputs cannot stand in for that named output contract.",
            "Declare and use an output type for the include result",
        ),
        "TYPE_INCLUDE_RESULT_SHAPE" => (
            "Output fields do not match the included relationships",
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
            "Output fields do not match the optional inverse relationship",
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
            "The field after `where:` must be declared on the entity being updated or deleted.",
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
            "Output fields do not match the nested relationship",
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
            "Output fields do not match the parent relationship",
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
            "A `supplied` check can name only a field declared on the patch input.",
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
            "A patch input must declare at least one entity field; an empty input can never describe a change.",
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
            "The field after `where:` must be declared on the entity being queried.",
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
            "Every field after `set:` must be declared on the entity being updated.",
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

fn cli_catalogue_copy(code: &str) -> Option<AuthoredCopy> {
    let (summary, reason, next) = match code {
        "CLI_CHECK_ARGUMENTS" => (
            "Check command has invalid arguments",
            "`check` accepts one project path plus the optional supported diagnostic presentation flags.",
            "Use `jadpo check <project>` with any presentation flags",
        ),
        "CLI_DEV_ARGUMENTS" => (
            "Dev command has invalid arguments",
            "`dev` accepts one project path and an optional JSON diagnostic format; other positional arguments are not defined.",
            "Use `jadpo dev <project> [--diagnostic-format=json]`",
        ),
        "CLI_DEV_BUN_START_FAILED" => (
            "Bun could not start the generated runtime",
            "The dev command built the target but could not launch `bun --no-install`, so no runtime process is available.",
            "Confirm Bun is installed and executable, then run dev again",
        ),
        "CLI_DEV_PORT_INVALID" => (
            "Development port is invalid",
            "The `PORT` environment value must be an integer from 1 through 65535 before the generated runtime can bind it.",
            "Set `PORT` to an available non-zero TCP port",
        ),
        "CLI_DEV_READINESS_TIMEOUT" => (
            "Generated runtime did not become ready",
            "The runtime process stayed alive but its local `/health` endpoint did not report readiness within the bounded startup window.",
            "Inspect the runtime startup output and verify the configured port is available",
        ),
        "CLI_DEV_RUNTIME_EXITED" => (
            "Generated runtime exited unexpectedly",
            "The dev supervisor observed the Bun process terminate before or after readiness, so it can no longer serve the checked build.",
            "Inspect the runtime output, correct the reported fault, and let dev rebuild",
        ),
        "CLI_DEV_RUNTIME_STATUS_FAILED" => (
            "Generated runtime status could not be read",
            "The dev supervisor could not determine whether the child runtime was still running, so it cannot safely report readiness.",
            "Stop the dev session and start a fresh runtime process",
        ),
        "CLI_DEV_TARGET_MISSING" => (
            "Generated runtime entrypoint is missing",
            "The dev command could not resolve `build/target/app.ts`, so there is no checked artifact to execute.",
            "Run a successful build for the project and start dev again",
        ),
        "CLI_FMT_ARGUMENTS" => (
            "Format command has invalid arguments",
            "`fmt` accepts one project path and only the optional `--check` mode.",
            "Use `jadpo fmt <project> [--check]`",
        ),
        "CLI_INCIDENT_ARGUMENTS" => (
            "Incident command has invalid arguments",
            "Local enrichment requires exactly the checked project and one secret-safe runtime event JSON file.",
            "Use `jadpo incident <project> <event-json-file>`",
        ),
        "CLI_INCIDENT_INVALID" => (
            "Runtime event is not a valid enrichment input",
            "The file must be a bounded `operational_log_event` with a supported runtime classification, semantic operation identifier, and generated request identifier.",
            "Provide an unchanged secret-safe event emitted by the generated runtime",
        ),
        "CLI_INCIDENT_MANIFEST_INVALID" => (
            "Local build manifest is not valid JSON",
            "Incident enrichment cannot safely map an operation to source when `build/app.meta.json` is malformed.",
            "Rebuild the exact project revision to regenerate the manifest",
        ),
        "CLI_INCIDENT_MANIFEST_MISSING" => (
            "Local build manifest is missing",
            "Incident enrichment needs the checked operation-to-source map from `build/app.meta.json` and never infers source locations from event payloads.",
            "Run `jadpo build` for the exact source revision before enrichment",
        ),
        "CLI_INCIDENT_MANIFEST_STALE" => (
            "Local build manifest does not match current sources",
            "The manifest's checked source revision differs from the project now on disk, so its operation locations are not trustworthy.",
            "Build the current sources or restore the source revision that produced the event",
        ),
        "CLI_INCIDENT_OPERATION_UNKNOWN" => (
            "Runtime operation is absent from the local manifest",
            "The event names a semantic operation that the matching local build metadata does not contain.",
            "Use the manifest and sources from the build that emitted this event",
        ),
        "CLI_INCIDENT_READ_FAILED" => (
            "Runtime event file could not be read",
            "The incident command could not load the selected local event JSON file.",
            "Restore read access and pass the correct event file path",
        ),
        "CLI_LSP_ARGUMENTS" => (
            "Language-server command has invalid arguments",
            "`jadpo lsp` runs over standard input and output and does not accept a project path or positional options.",
            "Run `jadpo lsp` without additional arguments",
        ),
        "CLI_PROJECT_REQUIRED" => (
            "Command requires a project path",
            "This command needs a Jadpo file or project directory before it can discover, check, or generate sources.",
            "Provide the project path after the command",
        ),
        "CLI_SCHEMA_COMMAND_REQUIRED" => (
            "Schema command requires a subcommand",
            "`schema` groups explicit identity, decision, plan, SQL-review, and index operations; it does not perform a default mutation.",
            "Choose a supported `jadpo schema` subcommand and project",
        ),
        "CLI_SCHEMA_DECISION_ARGUMENTS" => (
            "Schema decision command has invalid arguments",
            "Decision templates and validation require the current project, an `--against` snapshot, and the appropriate output or decisions file.",
            "Use the documented `schema decision-template` or `schema decision-check` form",
        ),
        "CLI_SCHEMA_DIFF_ARGUMENTS" => (
            "Schema diff command has invalid arguments",
            "A schema diff compares the current project against one explicit immutable snapshot.",
            "Use `jadpo schema diff <project> --against <snapshot>`",
        ),
        "CLI_SCHEMA_INDEX_ACCEPT_ARGUMENTS" => (
            "Index acceptance requires a recommendation path",
            "The checked edit must identify one current recommendation by its `Entity.field` path.",
            "Use `jadpo schema index-accept <project> <Entity.field>`",
        ),
        "CLI_SCHEMA_PLAN_ARGUMENTS" => (
            "Schema plan command has invalid arguments",
            "A migration plan requires an earlier snapshot, reviewed decisions, an explicit adapter, and an output file.",
            "Use `jadpo schema plan <project> --against <snapshot> --decisions <file> --adapter <adapter> <output>`",
        ),
        "CLI_SCHEMA_RENAME_ARGUMENTS" => (
            "Schema rename command has invalid arguments",
            "Identity-preserving rename requires a kind plus exact old and new semantic paths.",
            "Use `jadpo schema rename <project> <entity|field> <old> <new>`",
        ),
        "CLI_SCHEMA_SNAPSHOT_ARGUMENTS" => (
            "Schema snapshot command requires an output path",
            "The immutable identity snapshot is written only to an explicit destination and never overwrites the registry implicitly.",
            "Use `jadpo schema snapshot <project> <output>`",
        ),
        "CLI_SCHEMA_SQL_ARGUMENTS" => (
            "Schema SQL command has invalid arguments",
            "A SQL review requires an earlier snapshot, reviewed decisions, an explicit adapter, and an output file.",
            "Use `jadpo schema sql <project> --against <snapshot> --decisions <file> --adapter <adapter> <output>`",
        ),
        "CLI_UNKNOWN_COMMAND" => (
            "Unknown Jadpo command",
            "The first argument does not name a supported top-level Jadpo operation.",
            "Run `jadpo help` and choose a listed command",
        ),
        "CLI_UNKNOWN_SCHEMA_COMMAND" => (
            "Unknown schema subcommand",
            "The selected name is not one of the explicit schema identity, decision, planning, SQL-review, or index operations.",
            "Run `jadpo help` and choose a supported schema subcommand",
        ),
        "CLI_WATCH_ARGUMENTS" => (
            "Watch command has invalid arguments",
            "`watch` accepts one project path and an optional JSON diagnostic format; other positional arguments are not defined.",
            "Use `jadpo watch <project> [--diagnostic-format=json]`",
        ),
        "CLI_WATCH_INPUT_READ_FAILED" => (
            "Watched project inputs could not be read",
            "The watcher could not enumerate the project or read a tracked source while creating a stable change snapshot.",
            "Restore project read access; watching will retry on the next change",
        ),
        "CLI_WATCH_OUTPUT_FAILED" => (
            "Watch lifecycle event could not be written",
            "The watcher could not send its human or JSON event to the output stream, so consumers may have an incomplete revision sequence.",
            "Restart the watch process with a writable output stream",
        ),
        _ => return None,
    };
    Some(AuthoredCopy {
        summary,
        reason,
        next,
    })
}

fn migration_workflow_catalogue_copy(code: &str) -> Option<AuthoredCopy> {
    let (summary, reason, next) = match code {
        "MIG_DECISION_ARTIFACT_EXISTS" => (
            "Migration decision file already exists",
            "Decision templates are immutable review inputs and are never overwritten because doing so could erase authored approval evidence.",
            "Choose a new output path or review the existing decision file",
        ),
        "MIG_DECISION_ARTIFACT_INVALID" => (
            "Migration decision file is invalid",
            "The decision JSON does not match the versioned schema required to bind strategies and evidence to exact schema changes.",
            "Regenerate a decision template and transfer reviewed decisions into its schema",
        ),
        "MIG_DECISION_ARTIFACT_READ_FAILED" => (
            "Migration decision file could not be read",
            "The selected reviewed decision artifact was unavailable before validation could bind it to the current change set.",
            "Restore read access and retry with the reviewed decision file",
        ),
        "MIG_DECISION_ARTIFACT_WRITE_FAILED" => (
            "Migration decision template could not be written",
            "The compiler derived the required decisions but could not write the immutable review artifact.",
            "Choose a writable output path and generate the template again",
        ),
        "MIG_DECISION_CHANGE_SET_STALE" => (
            "Migration decisions refer to a different change set",
            "The decision file contains a fingerprint of the exact schema changes it approved. It cannot be reused after those changes differ.",
            "Regenerate the template and review every decision for the current change set",
        ),
        "MIG_DECISION_DUPLICATE" => (
            "Schema change has more than one migration decision",
            "Each identity and change pair must select one strategy so planning has a single reviewed instruction.",
            "Keep one decision for this schema change",
        ),
        "MIG_DECISION_EVIDENCE_DUPLICATE" => (
            "Migration decision repeats an evidence kind",
            "Each strategy-specific evidence kind has one value; duplicates make the reviewed justification ambiguous.",
            "Keep one value for each required evidence kind",
        ),
        "MIG_DECISION_EVIDENCE_EMPTY" => (
            "Migration decision contains empty evidence",
            "A required evidence field must contain a meaningful reviewed value rather than an empty or whitespace-only placeholder.",
            "Provide the reviewed evidence value",
        ),
        "MIG_DECISION_EVIDENCE_MISSING" => (
            "Migration decision is missing required evidence",
            "The selected strategy requires specific evidence before the compiler can record it as an approved migration instruction.",
            "Supply every evidence kind required by the selected strategy",
        ),
        "MIG_DECISION_EVIDENCE_UNEXPECTED" => (
            "Migration decision contains unsupported evidence",
            "The supplied evidence kind is not part of the selected strategy's review contract and cannot broaden that contract implicitly.",
            "Remove the unsupported evidence or choose the matching strategy",
        ),
        "MIG_DECISION_MISSING" => (
            "Schema change has no migration decision",
            "This change crosses an approval boundary and cannot be planned until a human selects a strategy and supplies its required evidence.",
            "Review the generated template and add the missing human-owned decision",
        ),
        "MIG_DECISION_STRATEGY_INVALID" => (
            "Migration strategy is not valid for this change",
            "Allowed strategies depend on the exact change and disposition; an unrelated strategy cannot be applied by name alone.",
            "Choose one of the strategies generated for this schema change",
        ),
        "MIG_DECISION_UNEXPECTED" => (
            "Decision file contains an unrelated schema change",
            "The identity and change pair is not among the decisions required by the bound current change set.",
            "Remove the unrelated decision or regenerate the template for the intended change set",
        ),
        "MIG_DECISION_UNRESOLVED" => (
            "Migration decision still needs a strategy",
            "A generated `null` strategy is an explicit review placeholder and cannot authorize planning or SQL generation.",
            "Have the responsible human select and evidence an allowed strategy",
        ),
        "MIG_IDENTITY_ADDITIONS_HAVE_REMOVAL" => (
            "Schema additions cannot be registered while identities are missing",
            "The live schema removed or renamed a registered identity; an additions-only command cannot decide that lifecycle change.",
            "Resolve renames or reviewed removals before registering additions",
        ),
        "MIG_IDENTITY_DUPLICATE_ID" => (
            "Schema registry repeats a stable identity",
            "One stable identifier cannot belong to multiple schema entries because diffs would no longer track a single object across renames.",
            "Restore the compiler-owned registry from a valid version or reinitialize before deployment",
        ),
        "MIG_IDENTITY_DUPLICATE_PATH" => (
            "Schema registry repeats a semantic path",
            "Each entity, field, constraint, reference, and index path must map to exactly one stable identity.",
            "Restore the compiler-owned registry or resolve the duplicate through schema commands",
        ),
        "MIG_IDENTITY_PHYSICAL_NAME" => (
            "Schema identity has no physical storage name",
            "Every persistent identity needs a stable physical name so migration SQL can address the same database object after logical renames.",
            "Restore a valid compiler-owned registry entry",
        ),
        "MIG_IDENTITY_REGISTRY_DRIFT" => (
            "Schema identities differ from checked source",
            "A persistent identity was added, removed, or renamed outside the explicit schema registry workflow, so intent cannot be inferred safely.",
            "Use the matching schema add or rename command, or restore the source declaration",
        ),
        "MIG_IDENTITY_REGISTRY_EXISTS" => (
            "Schema identity registry already exists",
            "Initialization never overwrites persistent identities because regeneration could silently assign new identities to deployed objects.",
            "Validate and use the existing registry",
        ),
        "MIG_IDENTITY_REGISTRY_INVALID" => (
            "Schema identity registry is invalid",
            "The registry JSON does not match the versioned compiler-owned schema required for stable migration identity.",
            "Restore the registry from version control or a verified backup",
        ),
        "MIG_IDENTITY_REGISTRY_MISSING" => (
            "Schema identity registry is missing",
            "This schema operation requires persistent identities, but the project has not initialized its compiler-owned registry.",
            "Run `jadpo schema init <project>` before this operation",
        ),
        "MIG_IDENTITY_REGISTRY_NOT_CANONICAL" => (
            "Schema identity registry was edited outside Jadpo",
            "The parsed entries do not serialize to the exact canonical compiler-owned representation, so manual edits cannot be trusted as lifecycle operations.",
            "Restore the canonical file and use schema commands for changes",
        ),
        "MIG_IDENTITY_REGISTRY_OWNER" => (
            "Schema identity has the wrong entity owner",
            "A field, constraint, reference, or index entry must carry the stable identity of the entity that owns its semantic path.",
            "Restore a valid registry or reapply the change through schema commands",
        ),
        "MIG_IDENTITY_REGISTRY_READ_FAILED" => (
            "Schema identity file could not be read",
            "The registry or snapshot path was unavailable, so the compiler cannot validate persistent identity before changing schema artifacts.",
            "Restore read access and retry the schema operation",
        ),
        "MIG_IDENTITY_REGISTRY_WRITE_FAILED" => (
            "Schema identity registry could not be written",
            "A checked initialization, addition, or rename could not persist its canonical identity update.",
            "Restore write access and retry the same schema operation",
        ),
        "MIG_IDENTITY_RENAME_KIND" => (
            "Schema rename kind is not supported",
            "Identity-preserving rename currently accepts only an `entity` or `field` semantic path.",
            "Choose `entity` or `field` as the rename kind",
        ),
        "MIG_IDENTITY_RENAME_SOURCE_UNKNOWN" => (
            "Schema rename source identity does not exist",
            "The old semantic path is not registered under the selected kind, so there is no stable identity to carry forward.",
            "Use the registered old path and matching kind",
        ),
        "MIG_IDENTITY_RENAME_TARGET_EXISTS" => (
            "Schema rename target already has an identity",
            "Moving the old stable identity onto an already registered target would merge two distinct persistent objects.",
            "Choose the unregistered renamed target or resolve the existing identity first",
        ),
        "MIG_IDENTITY_RENAME_TARGET_UNKNOWN" => (
            "Schema rename target is absent from checked source",
            "The new semantic path must already exist in the source before its old stable identity can be reassigned.",
            "Rename the source declaration first, then update its registry identity",
        ),
        "MIG_IDENTITY_SNAPSHOT_EXISTS" => (
            "Schema snapshot output already exists",
            "Snapshots are immutable comparison inputs and are never overwritten after review or archival.",
            "Choose a new output path or use the existing snapshot",
        ),
        "MIG_IDENTITY_SNAPSHOT_INVALID" => (
            "Schema snapshot is invalid",
            "The snapshot JSON does not match the versioned format Jadpo needs to compare stored schema definitions.",
            "Create a new snapshot with `jadpo schema snapshot`",
        ),
        "MIG_IDENTITY_SNAPSHOT_MISSING" => (
            "Previous schema snapshot is missing",
            "The requested comparison needs an immutable earlier snapshot, but the selected path does not exist.",
            "Provide the verified snapshot path",
        ),
        "MIG_IDENTITY_SNAPSHOT_NOT_CANONICAL" => (
            "Previous schema snapshot is not canonical",
            "The parsed snapshot differs from Jadpo's exact serialization, indicating it was edited or produced by an incompatible process.",
            "Recreate the snapshot using `jadpo schema snapshot`",
        ),
        "MIG_IDENTITY_SNAPSHOT_SHAPE_MISSING" => (
            "Snapshot entry is missing its saved definition",
            "Each snapshot entry must include its previous fields and storage rules so Jadpo can identify what changed and plan a review.",
            "Regenerate the snapshot from a valid identity registry",
        ),
        "MIG_IDENTITY_SNAPSHOT_WRITE_FAILED" => (
            "Schema snapshot could not be written",
            "The compiler derived a canonical immutable snapshot but could not create the selected output file.",
            "Choose a writable output path and take the snapshot again",
        ),
        "MIG_PLAN_ADAPTER_INVALID" => (
            "Migration adapter is not supported",
            "Migration planning and SQL review currently require an explicit `postgres` or `sqlite` adapter because their operations differ.",
            "Choose `postgres` or `sqlite`",
        ),
        "MIG_PLAN_DECISION_REJECTS_CHANGE" => (
            "Reviewed decision rejects this schema change",
            "The human-selected `reject` strategy intentionally denies this change, so the compiler will not turn it into a migration plan.",
            "Restore the previous schema definition or obtain a new reviewed decision for another allowed strategy",
        ),
        "MIG_PLAN_EXISTS" => (
            "Migration plan output already exists",
            "Plans are immutable review artifacts and are never overwritten because replacement could invalidate an approval trail.",
            "Choose a new output path or review the existing plan",
        ),
        "MIG_PLAN_WRITE_FAILED" => (
            "Migration plan could not be written",
            "The compiler validated the bound decisions and derived the non-executable plan but could not create its output file.",
            "Choose a writable output path and generate the plan again",
        ),
        _ => return None,
    };
    Some(AuthoredCopy {
        summary,
        reason,
        next,
    })
}

fn migration_sql_catalogue_copy(code: &str) -> Option<AuthoredCopy> {
    let (summary, reason, next) = match code {
        "MIG_SQL_CHANGE_UNSUPPORTED" => (
            "Schema change has no SQL review generator",
            "The non-executable migration plan can describe this reviewed change, but the selected adapter cannot yet derive safe forward and rollback SQL for it.",
            "Review the migration plan and author the unsupported operation outside generated SQL",
        ),
        "MIG_SQL_EXPRESSION_UNSUPPORTED" => (
            "Backfill expression cannot be compiled to SQL",
            "Generated migration SQL currently accepts only the explicit `literal(...)` evidence form; arbitrary application expressions are not executed during migration.",
            "Use a typed `literal(...)` backfill value or keep the step manual",
        ),
        "MIG_SQL_FIELD_OWNER_MISSING" => (
            "Migration field has no resolvable owning table",
            "The field snapshot or identity registry does not link this persistent field to a registered entity table, so SQL cannot address it safely.",
            "Restore and validate the schema identity registry before regenerating SQL",
        ),
        "MIG_SQL_FIELD_SHAPE_INVALID" => (
            "Saved field definition is incomplete",
            "The snapshot does not contain the field type and nullability Jadpo needs to choose the correct SQL operation.",
            "Regenerate valid schema identities and snapshots before planning this change",
        ),
        "MIG_SQL_LITERAL_INVALID" => (
            "Backfill literal is invalid for the field type",
            "The reviewed literal cannot be parsed as the target Text, UUID, DateTime, Int, Decimal, or Boolean value without changing its meaning.",
            "Provide a valid typed literal for the target field",
        ),
        "MIG_SQL_PREDICATE_UNSUPPORTED" => (
            "Validation check cannot be compiled to SQL",
            "Making a nullable field required needs the exact `not_null` check. Jadpo cannot treat another check as proof that existing rows are safe.",
            "Use `not_null` evidence or keep the narrowing outside generated SQL",
        ),
        "MIG_SQL_REVIEW_EXISTS" => (
            "Migration SQL review output already exists",
            "SQL reviews are immutable, non-executable approval artifacts and are never overwritten after generation.",
            "Choose a new output path or review the existing SQL artifact",
        ),
        "MIG_SQL_REVIEW_WRITE_FAILED" => (
            "Migration SQL review could not be written",
            "The compiler derived forward and rollback statements but could not create the immutable review output.",
            "Choose a writable output path and generate the SQL review again",
        ),
        "MIG_SQL_SQLITE_CONSTRAINT_MISSING" => (
            "SQLite rebuild cannot resolve a constraint identity",
            "Recreating the table requires the stable physical name of each primary, unique, or compound-unique constraint, and one is absent from the registry.",
            "Restore and validate the identity registry before regenerating the SQLite review",
        ),
        "MIG_SQL_SQLITE_ENTITY_MISSING" => (
            "SQLite rebuild cannot find the current entity",
            "The registered table does not match a checked entity declaration with all fields needed to recreate it.",
            "Restore consistency between source and the identity registry",
        ),
        "MIG_SQL_SQLITE_FIELD_IDENTITY_MISSING" => (
            "SQLite rebuild cannot resolve a field identity",
            "Copying rows into a rebuilt table requires each retained field's stable physical column name, and one is absent from the registry.",
            "Restore and validate the field identities before regenerating SQL",
        ),
        "MIG_SQL_SQLITE_FIELD_MISSING" => (
            "SQLite rebuild cannot find the current field",
            "A changed snapshot field does not resolve to a checked field on the entity being rebuilt, so its source and destination columns cannot be proven.",
            "Restore consistency between the snapshot, source, and identity registry",
        ),
        "MIG_SQL_SQLITE_INDEX_MISSING" => (
            "SQLite rebuild cannot resolve an index identity",
            "Recreating the table requires the stable physical name of every retained index, and one is absent from the registry.",
            "Restore and validate the index identities before regenerating SQL",
        ),
        "MIG_SQL_SQLITE_REBUILD_REQUIRED" => (
            "SQLite change requires a table rebuild",
            "SQLite cannot express this field alteration as the direct `ALTER TABLE` operation used by the general path; it must be grouped into a checked rebuild.",
            "Generate this change as a supported isolated SQLite rebuild set",
        ),
        "MIG_SQL_SQLITE_REBUILD_UNSUPPORTED" => (
            "This combination of SQLite table changes is not supported",
            "The reviewed changes combine fields, ownership, modifiers, or constraints that Jadpo cannot yet reproduce safely in one table rebuild.",
            "Split the change into supported steps or author and review the rebuild manually",
        ),
        "MIG_SQL_SQLITE_REFERENCE_MISSING" => (
            "SQLite rebuild cannot resolve a reference target",
            "Recreating a foreign key requires registered physical identities for its target table and column, and that mapping is incomplete.",
            "Restore and validate relationship identities before regenerating SQL",
        ),
        "MIG_SQL_SQLITE_RENAMED_TABLE_UNSUPPORTED" => (
            "SQLite rebuild cannot target a logically renamed table",
            "The bounded rebuild generator does not combine table-history resolution with a physical table recreation in one reviewed step.",
            "Separate the rename lifecycle from the rebuild or author the reviewed SQL manually",
        ),
        "MIG_SQL_STRATEGY_UNSUPPORTED" => (
            "Reviewed migration strategy has no SQL implementation",
            "The selected strategy is valid for planning but does not match the bounded adapter operation required for this SQL review.",
            "Choose a SQL-supported strategy through a new review or keep the step manual",
        ),
        "MIG_SQL_TYPE_UNSUPPORTED" => (
            "Field type has no migration SQL mapping",
            "Jadpo does not know which database column type safely represents this named field type for the selected database.",
            "Use a supported storage representation or author and review the type migration manually",
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
        .or_else(|| toolchain_catalogue_copy(code))
        .or_else(|| cli_catalogue_copy(code))
        .or_else(|| migration_workflow_catalogue_copy(code))
        .or_else(|| migration_sql_catalogue_copy(code));
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
        "FAIL_ATTEMPT_REQUIRED" => "This operation can fail and requires `attempt`".to_owned(),
        "FAIL_STALE_DECLARATION" => "A listed failure can never happen".to_owned(),
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
        "FAIL_ATTEMPT_REQUIRED" => "`attempt` makes it clear that a declared failure may leave the current function or action.".to_owned(),
        "FAIL_STALE_DECLARATION" => "Every failure after `fails` must be able to leave the function or action through a call or `reject`.".to_owned(),
        "FAIL_CONTEXT_FIELD_OVERLAP" => "The same field cannot be returned to callers and hidden from them at the same time.".to_owned(),
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
            "EFFECT_FUNCTION_CALLS_ACTION" => vec!["callable", "name"],
            "EFFECT_FUNCTION_PERSISTENCE" => vec!["callable"],
            "FAIL_ATTEMPT_REQUIRED" => vec!["usage", "operation"],
            "FAIL_CONTEXT_FIELD_OVERLAP" => vec!["failure", "field"],
            "FAIL_DUPLICATE_CODE" => vec!["failure", "otherFailure", "codeValue"],
            "FAIL_DUPLICATE_CONTEXT_FIELD" | "FAIL_UNKNOWN_CONTEXT_FIELD" => {
                vec!["failure", "field"]
            }
            "FAIL_DUPLICATE_DECLARATION" | "FAIL_UNDECLARED_PROPAGATION" => {
                vec!["callable", "failure"]
            }
            "FAIL_MISSING_CONTEXT_FIELD" => vec!["failure", "field", "scope"],
            "FAIL_MUTATION_CONFLICT_NOT_CONFLICT"
            | "FAIL_PATCH_EMPTY_NOT_INVALID_VALUE"
            | "FAIL_REQUIRED_MUTATION_NOT_NOT_FOUND"
            | "FAIL_REQUIRED_QUERY_NOT_NOT_FOUND" => {
                vec!["failure", "expected", "received", "usage"]
            }
            "FAIL_STALE_DECLARATION" => {
                vec!["callable", "declared", "reachable", "failures"]
            }
            "CLI_INCIDENT_REVISION_MISMATCH" => vec!["eventRevision", "localRevision"],
            "ROUTE_AUTH_VALUE_INVALID" => vec!["route", "found"],
            "SEM_DUPLICATE_DECLARATION" => vec!["name"],
            "SEM_UNKNOWN_NAME" | "SEM_WRONG_NAME_KIND" => vec![
                "name",
                "expected",
                "actualKind",
                "examples",
                "suggestedName",
                "usage",
            ],
            "SEM_UNKNOWN_CALLEE" => vec!["name", "expected", "suggestedName", "usage"],
            "SEM_NOT_CALLABLE" => vec!["name", "expected", "actualKind"],
            "TYPE_MISMATCH" | "TYPE_SIBLING_MISMATCH" => vec!["expected", "received"],
            "TYPE_ARGUMENT_COUNT" | "TYPE_CONSTRUCTOR_ARGUMENT_COUNT" => {
                vec!["name", "expectedCount", "receivedCount"]
            }
            "TYPE_CONSTRUCTOR_INPUT" => vec!["name", "expected", "received"],
            "TYPE_MISSING_FIELD" | "TYPE_MISSING_VARIANT_FIELD" => {
                vec!["subject", "field"]
            }
            "TYPE_UNKNOWN_FIELD" | "TYPE_UNKNOWN_VARIANT_FIELD" => {
                vec!["subject", "field", "suggestedName"]
            }
            "TYPE_UNKNOWN_VALUE" => vec!["name", "suggestedName"],
            "TYPE_UNKNOWN_ENUM_VARIANT" => vec!["subject", "name", "suggestedName"],
            "TYPE_FIELD_ON_NON_RECORD" | "TYPE_NULLABLE_SELECTION" => {
                vec!["received", "field"]
            }
            "TYPE_PRIMITIVE_SIGNATURE" => vec!["received", "usage"],
            "TYPE_INVALID_LITERAL" => vec!["subject", "constraint"],
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

fn semantic_expected_reason(
    expected: &str,
    examples: Option<&str>,
    suggestion: Option<&str>,
    usage: Option<&str>,
) -> String {
    let mut reason = match expected {
        "predefined category" => examples.map_or_else(
            || "`kind` must be a predefined category.".to_owned(),
            |values| format!("`kind` must be a predefined category, such as {values}."),
        ),
        other => usage.map_or_else(
            || format!("Jadpo expected a {other} here."),
            |usage| format!("This {usage} needs a {other}."),
        ),
    };
    if let Some(suggestion) = suggestion {
        reason.push_str(&format!(" Did you mean `{suggestion}`?"));
    }
    reason
}

fn semantic_unknown_name_guidance(name: &str, expected: &str, suggestion: Option<&str>) -> String {
    match expected {
        "type" => suggestion.map_or_else(
            || format!("Define `{name}` as a type or import the type"),
            |suggested| format!("Use `{suggested}`, define `{name}` as a type, or import the type"),
        ),
        "declared failure" => suggestion.map_or_else(
            || format!("Declare `{name}` as a failure or import the failure"),
            |suggested| format!("Use `{suggested}`, declare `{name}` as a failure, or import it"),
        ),
        "predefined category" => "Use one of Jadpo's predefined categories".to_owned(),
        _ => format!("Define `{name}` as a {expected} or import it"),
    }
}

fn semantic_replacement_guidance(expected: &str) -> String {
    match expected {
        "predefined category" => "Use one of Jadpo's predefined categories".to_owned(),
        "type" => "Use a type here".to_owned(),
        "declared failure" => "Use a declared failure here".to_owned(),
        "function or action" => "Use a function or action here".to_owned(),
        other => format!("Use a {other} here"),
    }
}

fn plural<'a>(singular: &'a str, count: &str) -> &'a str {
    if count == "1" {
        singular
    } else {
        match singular {
            "argument" => "arguments",
            other => other,
        }
    }
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
        catalogue_definition, catalogue_manifest_json, diagnostic_help_url, json_string,
        DecisionOwner, Diagnostic, DiagnosticFact, OperationalLogEvent, OperationalValue,
        PublicFailureResponse, PublicFailureValue, RepairKind, SafeIdentifier, Secret, SourceSpan,
        CATALOGUE_CODES,
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
    fn semantic_name_errors_explain_the_authored_source_in_plain_language() {
        let diagnostic = Diagnostic::error("SEM_UNKNOWN_NAME")
            .with_fact(DiagnosticFact::Name("CustomerID".to_owned()))
            .with_fact(DiagnosticFact::Expected("type".to_owned()))
            .with_fact(DiagnosticFact::SuggestedName("CustomerId".to_owned()))
            .with_fact(DiagnosticFact::Usage("field".to_owned()));

        assert_eq!(diagnostic.message, "`CustomerID` isn't defined");
        assert_eq!(
            diagnostic.reason,
            "This field needs a type. Did you mean `CustomerId`?"
        );
        assert_eq!(
            diagnostic.recommended_next_step.title,
            "Use `CustomerId`, define `CustomerID` as a type, or import the type"
        );
        assert_eq!(
            diagnostic_help_url(&diagnostic.help_id),
            "https://jadpo.dev/docs/diagnostics/semantic.unknown_name"
        );
    }

    #[test]
    fn type_errors_name_the_received_and_required_types() {
        let diagnostic = Diagnostic::error("TYPE_SIBLING_MISMATCH")
            .with_fact(DiagnosticFact::Expected("CustomerId".to_owned()))
            .with_fact(DiagnosticFact::Received("OrderId".to_owned()));

        assert_eq!(
            diagnostic.message,
            "`OrderId` can't be used where `CustomerId` is required"
        );
        assert_eq!(
            diagnostic.reason,
            "`OrderId` and `CustomerId` are separate named types, even if they build on the same base type."
        );
        assert_eq!(
            diagnostic.recommended_next_step.title,
            "Use a `CustomerId` value here"
        );
        assert_eq!(
            diagnostic.context,
            vec![
                ("expected".to_owned(), "CustomerId".to_owned()),
                ("received".to_owned(), "OrderId".to_owned())
            ]
        );
    }

    #[test]
    fn failure_errors_name_the_function_and_escaping_failure() {
        let diagnostic = Diagnostic::error("FAIL_UNDECLARED_PROPAGATION")
            .with_fact(DiagnosticFact::Callable("register".to_owned()))
            .with_fact(DiagnosticFact::Failure("RegistrationClosed".to_owned()));

        assert_eq!(
            diagnostic.message,
            "`RegistrationClosed` can leave `register` but isn't listed after `fails`"
        );
        assert_eq!(
            diagnostic.reason,
            "Callers of `register` need to know that `RegistrationClosed` can happen."
        );
        assert_eq!(
            diagnostic.recommended_next_step.title,
            "Add `RegistrationClosed` after `fails`, or handle it inside `register`"
        );
    }

    #[test]
    fn generated_catalogue_contains_every_representative_compiler_family() {
        let manifest = catalogue_manifest_json();
        for alias in [
            "SYN_UNEXPECTED_TOKEN",
            "TYPE_UNKNOWN_VALUE",
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
    fn warnings_do_not_claim_that_they_block_compilation() {
        let warning = Diagnostic::warning("INDEX_RECOMMENDATION_AVAILABLE");

        assert_eq!(
            warning.impact.behavioral,
            "The project can still build; this recommendation is advisory."
        );
        assert!(!warning.impact.behavioral.contains("blocked"));
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
    fn every_public_diagnostic_has_authored_human_readable_copy() {
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
            placeholders.is_empty(),
            "placeholder catalogue copy remains: {}",
            placeholders.join(", ")
        );
    }

    #[test]
    fn every_public_diagnostic_uses_source_level_plain_language() {
        let forbidden = [
            "This position",
            "grammar position",
            "semantic declaration kind",
            "wrong failure kind",
            "recoverable problem set",
            "non-callable declaration",
            "semantic representation",
            "operand shapes",
            "wrong shape",
            "Declared problem",
            "Reachable problem",
            "mathematical set",
            "callee",
            "nominal siblings",
            "nominal type",
            "semantic type domain",
        ];

        for code in CATALOGUE_CODES {
            let definition = catalogue_definition(code);
            let copy = format!(
                "{} {} {}",
                definition.summary, definition.reason, definition.recommended_title
            );
            for phrase in forbidden {
                assert!(
                    !copy.contains(phrase),
                    "{code}: public copy contains compiler-oriented phrase `{phrase}`"
                );
            }
        }
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
    fn every_cli_diagnostic_has_rule_specific_public_copy() {
        for code in CATALOGUE_CODES
            .iter()
            .filter(|code| code.starts_with("CLI_"))
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
    fn every_migration_identity_decision_and_plan_diagnostic_has_rule_specific_public_copy() {
        for code in CATALOGUE_CODES.iter().filter(|code| {
            code.strip_prefix("MIG_")
                .is_some_and(|remainder| !remainder.starts_with("SQL_"))
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
    fn every_migration_diagnostic_has_rule_specific_public_copy() {
        for code in CATALOGUE_CODES
            .iter()
            .filter(|code| code.starts_with("MIG_"))
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
    fn every_public_diagnostic_is_authored_and_has_conformance_evidence() {
        let failures = CATALOGUE_CODES
            .iter()
            .filter_map(|code| {
                let definition = catalogue_definition(code);
                let has_conformance_evidence = definition.fixtures.iter().any(|fixture| {
                    *fixture
                        != "jadpo/crates/diagnostics/src/lib.rs#every_catalogue_entry_is_renderable"
                });
                (!definition.authored_copy || !has_conformance_evidence).then_some(format!(
                    "{code}: copy={}, evidence={}",
                    if definition.authored_copy {
                        "authored"
                    } else {
                        "placeholder"
                    },
                    if has_conformance_evidence {
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
