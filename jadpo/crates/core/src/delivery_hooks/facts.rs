//! Incomplete facts captured at the exact census-allowlisted mutation sites.
//! Constructors are restricted to that checker; no source descriptor can issue
//! a checked binding by naming a hook or copying these inspection facts.
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HookKind {
    Create,
    Patch,
}
impl HookKind {
    fn name(self) -> &'static str {
        match self {
            Self::Create => "direct_create",
            Self::Patch => "direct_patch",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct HookSiteCandidate {
    kind: HookKind,
    callable: String,
    parameter_index: usize,
    parameter_name: String,
    parameter_type: String,
    input_field: String,
    due_field: String,
    sent_field: String,
    open_field_variant: Option<(String, String)>,
}
impl HookSiteCandidate {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        kind: HookKind,
        callable: String,
        parameter_index: usize,
        parameter_name: String,
        parameter_type: String,
        input_field: String,
        due_field: String,
        sent_field: String,
        open_field_variant: Option<(String, String)>,
    ) -> Self {
        Self {
            kind,
            callable,
            parameter_index,
            parameter_name,
            parameter_type,
            input_field,
            due_field,
            sent_field,
            open_field_variant,
        }
    }
    fn facts(&self) -> Value {
        json!({"site_identity": format!("{}.{}", self.callable, self.kind.name()),
            "callable": self.callable, "kind": self.kind.name(),
            "parameter_identity": format!("{}.parameter.{}", self.callable, self.parameter_index),
            "parameter_name": self.parameter_name, "parameter_type": self.parameter_type,
            "input_due_field": self.input_field, "bound_due_field": self.due_field,
            "bound_sent_field": self.sent_field,
            "open_initial": self.open_field_variant.as_ref().map(|(field,variant)| json!({"field":field,"variant":variant})),
            "sent_initial": if self.kind == HookKind::Create { Some("none") } else { None },
            "sent_reset": if self.kind == HookKind::Patch { Some("none_when_same_parameter_due_supplied") } else { None },
            "transactional_revision_evidence": "unproved", "runtime_supported": false})
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveryHookCandidate {
    job: String,
    source: String,
    create: HookSiteCandidate,
    patch: HookSiteCandidate,
}
impl DeliveryHookCandidate {
    pub(super) fn new(
        job: String,
        source: String,
        create: HookSiteCandidate,
        patch: HookSiteCandidate,
    ) -> Self {
        Self {
            job,
            source,
            create,
            patch,
        }
    }
    pub(crate) fn job_candidate(&self) -> &str {
        &self.job
    }
    pub(crate) fn source_candidate(&self) -> &str {
        &self.source
    }
    pub(crate) fn graph_callable_candidates(&self) -> [(&'static str, &str); 2] {
        [
            ("create_hook", self.create.callable.as_str()),
            ("patch_hook", self.patch.callable.as_str()),
        ]
    }
    pub fn candidate_facts(&self) -> Value {
        let mut facts = self.source_facts();
        facts["status"] = json!("incomplete_nonexecuting_candidate");
        facts
    }
    pub(crate) fn source_facts(&self) -> Value {
        json!({"job":self.job,
            "create":self.create.facts(), "patch":self.patch.facts()})
    }
}
