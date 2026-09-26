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
}

impl DiagnosticFact {
    fn into_pair(self) -> (&'static str, String) {
        match self {
            Self::Callable(value) => ("callable", value),
            Self::DeclaredProblems(value) => ("declared", value),
            Self::ReachableProblems(value) => ("reachable", value),
            Self::EventRevision(value) => ("eventRevision", value),
            Self::LocalRevision(value) => ("localRevision", value),
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
        Self {
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
        }
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
        if self.context.len() < 16 && definition.context_keys.contains(&key) {
            self.context.push((key.to_owned(), value));
        }
        self
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
    let human_owned = matches!(
        code,
        "JADPO_TARGET_AUTH_NOT_IMPLEMENTED"
            | "MIG_DECISION_UNRESOLVED"
            | "MIG_DECISION_MISSING"
            | "MIG_DECISION_EVIDENCE_MISSING"
            | "MIG_PLAN_DECISION_REJECTS_CHANGE"
    );
    let automatic = matches!(code, "FAIL_ATTEMPT_REQUIRED" | "FAIL_STALE_DECLARATION");
    let (repair_kind, decision_owner, recommended_title) = if human_owned {
        (
            RepairKind::HumanDecision,
            DecisionOwner::Human,
            "Request the named human-owned decision".to_owned(),
        )
    } else if automatic {
        (
            RepairKind::AutomaticFix,
            DecisionOwner::Compiler,
            match code {
                "FAIL_ATTEMPT_REQUIRED" => "Prefix the fallible expression with `attempt`",
                "FAIL_STALE_DECLARATION" => "Remove the stale `fails` entry",
                _ => "Apply the compiler-verified edit",
            }
            .to_owned(),
        )
    } else {
        (
            RepairKind::GuidedChoice,
            DecisionOwner::Agent,
            "Update the source to satisfy this rule".to_owned(),
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
        "CLI_INCIDENT_REVISION_MISMATCH" => {
            "Runtime event and local source revisions differ".to_owned()
        }
        "RUNTIME_UNHANDLED_FAULT" => "Generated runtime contained an unexpected fault".to_owned(),
        "RUNTIME_STARTUP_FAILED" => "Generated runtime failed during startup".to_owned(),
        _ => sentence_case_identifier(remainder),
    };
    let reason = match code {
        "FAIL_ATTEMPT_REQUIRED" => "Every expression with a recoverable problem set must make propagation visible with `attempt`.".to_owned(),
        "FAIL_STALE_DECLARATION" => "A callable's authored `fails` set must exactly equal its reachable unhandled problem set.".to_owned(),
        "FAIL_CONTEXT_FIELD_OVERLAP" => "A flat failure value cannot be assigned safely when the declaration gives the same field both public and internal disclosure.".to_owned(),
        "ROUTE_PATH_BINDING_MISSING" | "ROUTE_PATH_BINDING_EXTRA" => "Route placeholders and typed `path` fields must correspond exactly one-to-one.".to_owned(),
        "ROUTE_BEHAVIOUR_CONFLICT" | "ROUTE_BEHAVIOUR_REQUIRED" => "A route must select exactly one local inline action or one named `run:` invocation.".to_owned(),
        "CLI_INCIDENT_REVISION_MISMATCH" => "Local enrichment is trustworthy only when the runtime event and compiler graph identify the same checked source revision.".to_owned(),
        "RUNTIME_UNHANDLED_FAULT" => "An exception outside the declared domain-failure boundary was contained by the generated runtime.".to_owned(),
        "RUNTIME_STARTUP_FAILED" => "The generated runtime could not establish its startup contract and did not report readiness.".to_owned(),
        _ => format!("The compiler-enforced `{code}` invariant is not satisfied at this location."),
    };
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
            _ => Vec::new(),
        },
        fixtures: CATALOGUE_FIXTURES
            .iter()
            .filter_map(|(fixture_code, path)| (*fixture_code == code).then_some(*path))
            .collect(),
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
                "{{\"ruleId\":{},\"category\":{},\"summary\":{},\"reason\":{},\"recommendedNextStep\":{},\"repairKind\":{},\"decisionOwner\":{},\"contextSchema\":{{\"allowedKeys\":[{allowed_keys}],\"additionalProperties\":false}},\"helpId\":{},\"fixtures\":[{fixtures}],\"legacyAliases\":[{}]}}",
                json_string(&entry.rule_id),
                json_string(&entry.category),
                json_string(&entry.summary),
                json_string(&entry.reason),
                json_string(&entry.recommended_title),
                json_string(entry.repair_kind.as_str()),
                json_string(entry.decision_owner.as_str()),
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
        catalogue_definition, catalogue_manifest_json, DecisionOwner, Diagnostic,
        OperationalLogEvent, OperationalValue, PublicFailureResponse, PublicFailureValue,
        RepairKind, SafeIdentifier, Secret, SourceSpan,
    };

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
}
