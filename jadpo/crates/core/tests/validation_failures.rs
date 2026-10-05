//! Independent contract probes: docs/failure-model.md §§4–6, 12 and
//! docs/grammar-v0.1.md §§10–12. Expected sets are written explicitly; none are
//! obtained from the implementation or its generated target.
use jadpo_core::{analyze_sources, AnalyzedProject};
use jadpo_diagnostics::Diagnostic;
use jadpo_syntax::SourceFile;

const PRELUDE: &str = r#"
type ResultText = Text {}
type Flag = Bool {}
failure Missing { kind: NotFound code: "missing" }
failure Refused { kind: Rejected code: "refused" }
failure Mapped { kind: Conflict code: "mapped" }
function lookup(flag: Flag) fails Missing, Refused -> ResultText {
    if (flag == Flag(true)) { reject Missing } else { reject Refused }
}
"#;

fn analyze(source: &str) -> AnalyzedProject {
    analyze_sources(vec![SourceFile::new(
        "validation.jadpo".into(),
        source.into(),
    )])
    .expect("nonempty contract fixture is analyzable")
}
fn diagnostics(project: &AnalyzedProject) -> Vec<&Diagnostic> {
    project
        .syntax
        .diagnostics()
        .chain(project.semantics.diagnostics.iter())
        .chain(project.typing.diagnostics.iter())
        .chain(project.failures.diagnostics.iter())
        .collect()
}
fn clean(source: &str) -> AnalyzedProject {
    let project = analyze(source);
    assert!(
        diagnostics(&project).is_empty(),
        "unexpected diagnostics: {:#?}",
        diagnostics(&project)
    );
    project
}
fn failures<'a>(project: &'a AnalyzedProject, name: &str) -> Vec<&'a str> {
    project
        .failures
        .callables
        .iter()
        .find(|c| c.callable == name)
        .unwrap_or_else(|| panic!("missing callable {name}"))
        .failures
        .iter()
        .map(String::as_str)
        .collect()
}
fn error<'a>(project: &'a AnalyzedProject, code: &str) -> &'a Diagnostic {
    let matching: Vec<_> = diagnostics(project)
        .into_iter()
        .filter(|d| d.code == code)
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "expected one {code}; got {:#?}",
        diagnostics(project)
    );
    matching[0]
}

#[test]
fn outcome_decisions_have_exact_independent_failure_sets() {
    // Exhaust the recovery/propagation/mapping choices for two independent
    // failure arms. Each expected set is the union of the authored decisions.
    for (missing_arm, missing_set) in [
        ("ResultText(\"recovered\")", None),
        ("propagate", Some("Missing")),
        ("reject Mapped", Some("Mapped")),
    ] {
        for (refused_arm, refused_set) in [
            ("ResultText(\"recovered\")", None),
            ("propagate", Some("Refused")),
            ("reject Mapped", Some("Mapped")),
        ] {
            let mut expected: Vec<_> = missing_set.into_iter().chain(refused_set).collect();
            expected.sort();
            expected.dedup();
            let clause = if expected.is_empty() {
                String::new()
            } else {
                format!("fails {}", expected.join(", "))
            };
            let source = format!("{PRELUDE}\nfunction decide(flag: Flag) {clause} -> ResultText {{\nreturn match lookup(flag) {{ success(value) => value failure Missing => {missing_arm} failure Refused => {refused_arm} }}\n}}");
            let project = clean(&source);
            assert_eq!(
                failures(&project, "decide"),
                expected,
                "{missing_arm}; {refused_arm}"
            );
        }
    }
}

#[test]
fn handled_failures_do_not_reappear_in_transitive_callers_or_routes() {
    let source = format!(
        r#"{PRELUDE}
function decide(flag: Flag) fails Refused -> ResultText {{
    return match lookup(flag) {{
        success(value) => value
        failure Missing => ResultText("fallback")
        failure Refused => propagate
    }}
}}
action outer(flag: Flag) fails Refused -> ResultText {{ return attempt decide(flag) }}
value Input {{ flag: Flag }}
route POST /decide {{ auth: none input: Input output: ResultText run: outer(input.flag) }}
"#
    );
    let project = clean(&source);
    assert_eq!(failures(&project, "outer"), ["Refused"]);
    let routes: Vec<_> = project
        .failures
        .routes
        .iter()
        .map(|r| (r.failure.as_str(), r.http_status, r.derived))
        .collect();
    assert_eq!(routes, [("Refused", Some(422), true)]);
}

#[test]
fn missing_acknowledgement_has_call_span_and_is_repaired_by_attempt() {
    let source = format!("{PRELUDE}\nfunction caller(flag: Flag) fails Missing, Refused -> ResultText {{ return lookup(flag) }}");
    let project = analyze(&source);
    let diagnostic = error(&project, "FAIL_ATTEMPT_REQUIRED");
    let span = diagnostic.primary.as_ref().expect("call span");
    assert_eq!(&source[span.start..span.end], "lookup(flag)");
    assert!(!diagnostic.message.is_empty());
    assert!(!diagnostic.recommended_next_step.title.is_empty());
    clean(&source.replace("return lookup(flag)", "return attempt lookup(flag)"));
}

#[test]
fn missing_propagated_member_cannot_be_hidden_by_attempt() {
    let source = format!("{PRELUDE}\nfunction caller(flag: Flag) fails Missing -> ResultText {{ return attempt lookup(flag) }}");
    let project = analyze(&source);
    let diagnostic = error(&project, "FAIL_UNDECLARED_PROPAGATION");
    let span = diagnostic.primary.as_ref().expect("call span");
    assert_eq!(&source[span.start..span.end], "lookup(flag)");
    assert!(diagnostic
        .context
        .iter()
        .any(|(_, value)| value.contains("Refused")));
    clean(&source.replace(
        "caller(flag: Flag) fails Missing",
        "caller(flag: Flag) fails Missing, Refused",
    ));
}

#[test]
fn recovered_failure_is_stale_even_when_callee_declares_it() {
    let source = format!(
        r#"{PRELUDE}
function recover(flag: Flag) fails Missing -> ResultText {{
    return match lookup(flag) {{ success(value) => value failure Missing => ResultText("m") failure Refused => ResultText("r") }}
}}"#
    );
    let project = analyze(&source);
    error(&project, "FAIL_STALE_DECLARATION");
    clean(&source.replace("recover(flag: Flag) fails Missing", "recover(flag: Flag)"));
}

#[test]
fn outcome_exhaustiveness_is_checked_for_each_named_arm() {
    for (arms, code) in [
        ("failure Missing => ResultText(\"m\") failure Refused => ResultText(\"r\")", "FAIL_OUTCOME_SUCCESS_MISSING"),
        ("success(value) => value failure Missing => ResultText(\"m\")", "FAIL_OUTCOME_MISSING_ARM"),
        ("success(value) => value failure Missing => ResultText(\"m\") failure Missing => ResultText(\"m2\") failure Refused => ResultText(\"r\")", "FAIL_OUTCOME_DUPLICATE_ARM"),
        ("success(value) => value success(other) => other failure Missing => ResultText(\"m\") failure Refused => ResultText(\"r\")", "FAIL_OUTCOME_SUCCESS_DUPLICATE"),
        ("success(value) => value failure Missing => ResultText(\"m\") failure Refused => ResultText(\"r\") failure Mapped => ResultText(\"x\")", "FAIL_OUTCOME_UNKNOWN_ARM"),
    ] {
        let source = format!("{PRELUDE}\nfunction recover(flag: Flag) -> ResultText {{ return match lookup(flag) {{ {arms} }} }}");
        let project = analyze(&source);
        let diagnostic = error(&project, code);
        let span = diagnostic.primary.as_ref().expect("source anchored diagnostic");
        assert!(span.end > span.start && span.end <= source.len(), "{code}");
        assert!(!diagnostic.message.is_empty() && !diagnostic.recommended_next_step.title.is_empty());
    }
}

#[test]
fn pure_function_cannot_hide_action_effect_in_outcome_match() {
    let source = format!(
        r#"{PRELUDE}
action perform(flag: Flag) fails Missing, Refused -> ResultText {{ return attempt lookup(flag) }}
function disguise(flag: Flag) -> ResultText {{
    return match perform(flag) {{ success(value) => value failure Missing => ResultText("m") failure Refused => ResultText("r") }}
}}"#
    );
    let project = analyze(&source);
    let diagnostic = error(&project, "EFFECT_FUNCTION_CALLS_ACTION");
    let span = diagnostic.primary.as_ref().expect("effect invocation span");
    assert_eq!(&source[span.start..span.end], "perform(flag)");
    clean(&source.replace("function disguise", "action disguise"));
}

#[test]
fn failure_schemas_keep_public_and_internal_context_distinct() {
    let source = r#"
type Detail = Text {}
type Trace = Text {}
type ResultText = Text {}
failure Denied {
    kind: NotPermitted code: "denied"
    public { safe_reason: Detail }
    internal { audit_reason: Trace }
}
action deny(safe: Detail, trace: Trace) fails Denied -> ResultText {
    reject Denied { safe_reason: safe audit_reason: trace }
}
"#;
    let project = clean(source);
    let contract = &project.failures.contracts[0];
    assert_eq!(contract.public_fields, ["safe_reason"]);
    assert_eq!(contract.internal_fields, ["audit_reason"]);
    assert_eq!(contract.http_status, Some(403));
    let missing = analyze(&source.replace("audit_reason: trace", ""));
    error(&missing, "FAIL_MISSING_CONTEXT_FIELD");
    let extra = analyze(&source.replace(
        "audit_reason: trace",
        "audit_reason: trace unexpected: trace",
    ));
    error(&extra, "FAIL_UNKNOWN_CONTEXT_FIELD");
    let swapped = analyze(&source.replace(
        "safe_reason: safe audit_reason: trace",
        "safe_reason: trace audit_reason: safe",
    ));
    assert!(
        diagnostics(&swapped)
            .iter()
            .any(|d| d.code == "TYPE_MISMATCH"),
        "{:#?}",
        diagnostics(&swapped)
    );
}

#[test]
fn exact_reachable_failures_exclude_statements_after_return() {
    let source = r#"
type ResultText = Text {}
failure Missing { kind: NotFound code: "missing" }
function done() -> ResultText {
    return ResultText("complete")
    reject Missing
}
"#;
    let project = analyze(source);
    assert!(project.syntax.diagnostics().next().is_none());
    // An unreachable-code diagnostic is allowed; a claim that Missing can
    // propagate is not. The failure-model §5.2 contract says reachable.
    assert!(
        !diagnostics(&project)
            .iter()
            .any(|d| d.code == "FAIL_UNDECLARED_PROPAGATION"),
        "unreachable rejection included: {:#?}",
        diagnostics(&project)
    );
    assert!(failures(&project, "done").is_empty());
}

#[test]
fn all_terminating_branches_exclude_a_later_rejection() {
    let source = r#"
type ResultText = Text {}
type Flag = Bool {}
failure Missing { kind: NotFound code: "missing" }
function done(flag: Flag) -> ResultText {
    if (flag == Flag(true)) { return ResultText("left") } else { return ResultText("right") }
    reject Missing
}
"#;
    let project = analyze(source);
    assert!(project.syntax.diagnostics().next().is_none());
    assert!(
        !diagnostics(&project)
            .iter()
            .any(|d| d.code == "FAIL_UNDECLARED_PROPAGATION"),
        "unreachable branch continuation: {:#?}",
        diagnostics(&project)
    );
    assert!(failures(&project, "done").is_empty());
}

#[test]
fn argument_failures_are_not_caught_by_the_outer_invocation_match() {
    let source = r#"
type ResultText = Text {}
failure Missing { kind: NotFound code: "missing" }
failure Refused { kind: Rejected code: "refused" }
function argument() fails Missing -> ResultText { reject Missing }
function subject(value: ResultText) fails Refused -> ResultText { reject Refused }
function caller() fails Missing -> ResultText {
    return match subject(attempt argument()) {
        success(value) => value
        failure Refused => ResultText("recovered")
    }
}
"#;
    let project = clean(source);
    assert_eq!(failures(&project, "caller"), ["Missing"]);
    let missing = analyze(&source.replace("caller() fails Missing", "caller()"));
    error(&missing, "FAIL_UNDECLARED_PROPAGATION");
}

#[test]
fn recovery_expression_can_introduce_a_different_failure() {
    let source = r#"
type ResultText = Text {}
failure Missing { kind: NotFound code: "missing" }
failure Refused { kind: Rejected code: "refused" }
function original() fails Missing -> ResultText { reject Missing }
function fallback() fails Refused -> ResultText { reject Refused }
function caller() fails Refused -> ResultText {
    return match original() {
        success(value) => value
        failure Missing => attempt fallback()
    }
}
"#;
    let project = clean(source);
    assert_eq!(failures(&project, "caller"), ["Refused"]);
    let missing = analyze(&source.replace("caller() fails Refused", "caller()"));
    error(&missing, "FAIL_UNDECLARED_PROPAGATION");
    let unacknowledged = analyze(&source.replace("attempt fallback()", "fallback()"));
    error(&unacknowledged, "FAIL_ATTEMPT_REQUIRED");
}

#[test]
fn one_terminating_branch_does_not_hide_a_reachable_failure() {
    let source = r#"
type ResultText = Text {}
type Flag = Bool {}
failure Missing { kind: NotFound code: "missing" }
function choose(flag: Flag) fails Missing -> ResultText {
    if (flag == Flag(true)) { return ResultText("left") }
    reject Missing
}
"#;
    let project = clean(source);
    assert_eq!(failures(&project, "choose"), ["Missing"]);
    let undeclared = analyze(&source.replace("fails Missing", ""));
    error(&undeclared, "FAIL_UNDECLARED_PROPAGATION");
}

#[test]
fn stale_failure_edit_preserves_still_reachable_members() {
    let source = r#"
type ResultText = Text {}
failure Missing { kind: NotFound code: "missing" }
failure Refused { kind: Rejected code: "refused" }
function work() fails Missing, Refused -> ResultText { reject Refused }
"#;
    let project = analyze(source);
    let diagnostic = error(&project, "FAIL_STALE_DECLARATION");
    let span = diagnostic.primary.as_ref().expect("fails clause span");
    assert_eq!(&source[span.start..span.end], "fails Missing, Refused");
    let mut repaired = source.to_string();
    let mut edits = diagnostic.recommended_next_step.edits.clone();
    assert!(
        !edits.is_empty(),
        "stale-set diagnostic promises a mechanical repair"
    );
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.start));
    for edit in edits {
        repaired.replace_range(edit.start..edit.end, &edit.replacement);
    }
    let repaired_project = clean(&repaired);
    assert_eq!(failures(&repaired_project, "work"), ["Refused"]);
    assert!(repaired.contains("reject Refused"));
}

#[test]
fn secret_configuration_cannot_enter_either_failure_disclosure_channel() {
    // CONFIG-001 intentionally permits secrets only at compiler-approved
    // adapter sinks. Declaring an internal diagnostic field is not such a sink.
    for channel in ["public", "internal"] {
        let source = format!(
            r#"
type ApiKey = Text {{}}
type ResultText = Text {{}}
config Settings {{ api_key: ApiKey {{ binding: "API_KEY" secret: true }} }}
failure Denied {{ kind: Rejected code: "denied" {channel} {{ detail: ApiKey }} }}
action deny() fails Denied -> ResultText {{ reject Denied {{ detail: config.api_key }} }}
"#
        );
        let project = analyze(&source);
        let diagnostic = error(&project, "CONFIG_SECRET_FLOW");
        let span = diagnostic.primary.as_ref().expect("secret flow span");
        assert_eq!(&source[span.start..span.end], "config.api_key");
    }
}

#[test]
fn overlapping_public_and_internal_names_are_not_silently_disclosed() {
    let source = r#"
type Detail = Text {}
failure Denied {
    kind: Rejected code: "denied"
    public { detail: Detail }
    internal { detail: Detail }
}
"#;
    let project = analyze(source);
    let diagnostic = error(&project, "FAIL_CONTEXT_FIELD_OVERLAP");
    let span = diagnostic.primary.as_ref().expect("overlap field span");
    assert_eq!(&source[span.start..span.end], "detail");
    clean(&source.replace("internal { detail: Detail }", "internal { audit: Detail }"));
}

#[test]
fn public_error_documentation_omits_internal_context_but_audit_keeps_it() {
    let source = r#"
type Detail = Text {}
type Trace = Text {}
type ResultText = Text {}
value Input { safe: Detail trace: Trace }
failure Denied {
    kind: NotPermitted code: "denied"
    public { safe_reason: Detail }
    internal { audit_reason: Trace }
}
action deny(input: Input) fails Denied -> ResultText {
    reject Denied { safe_reason: input.safe audit_reason: input.trace }
}
route POST /deny { auth: none input: Input output: ResultText run: deny(input) }
"#;
    let project = clean(source);
    let artifacts =
        jadpo_core::derive_artifacts(std::path::Path::new("validation.jadpo"), &project);
    let public = &artifacts
        .iter()
        .find(|a| a.relative_path == "openapi/openapi.json")
        .expect("public API artifact")
        .contents;
    assert!(public.contains("safe_reason"));
    assert!(
        !public.contains("audit_reason"),
        "internal failure schema leaked into public contract"
    );
    let audit = &artifacts
        .iter()
        .find(|a| a.relative_path == "audit/failures.json")
        .expect("failure audit artifact")
        .contents;
    assert!(audit.contains("safe_reason") && audit.contains("audit_reason"));
}

#[test]
fn unreachable_calls_and_outcome_propagation_do_not_expand_failure_contracts() {
    for unreachable in [
        "return attempt lookup(flag)",
        "return match lookup(flag) { success(value) => value failure Missing => propagate failure Refused => propagate }",
    ] {
        let source = format!("{PRELUDE}\nfunction done(flag: Flag) -> ResultText {{ return ResultText(\"complete\") {unreachable} }}");
        let project = clean(&source);
        assert!(failures(&project, "done").is_empty());
        let stale = analyze(&source.replace("done(flag: Flag)", "done(flag: Flag) fails Missing, Refused"));
        error(&stale, "FAIL_STALE_DECLARATION");
        assert!(!diagnostics(&stale).iter().any(|d| d.code == "FAIL_UNDECLARED_PROPAGATION"));
    }
}

#[test]
fn unreachable_source_still_checks_effects_and_acknowledgement() {
    let source = format!(
        r#"{PRELUDE}
action perform(flag: Flag) fails Missing, Refused -> ResultText {{ return attempt lookup(flag) }}
function done(flag: Flag) -> ResultText {{
    return ResultText("complete")
    return perform(flag)
}}
"#
    );
    let project = analyze(&source);
    error(&project, "EFFECT_FUNCTION_CALLS_ACTION");
    error(&project, "FAIL_ATTEMPT_REQUIRED");
    assert!(!diagnostics(&project)
        .iter()
        .any(|d| d.code == "FAIL_UNDECLARED_PROPAGATION"));
}

#[test]
fn unreachable_rejection_still_checks_required_context() {
    let source = r#"
type ResultText = Text {}
failure Missing { kind: NotFound code: "missing" internal { detail: ResultText } }
function done() -> ResultText {
    return ResultText("complete")
    reject Missing
}
"#;
    let project = analyze(source);
    error(&project, "FAIL_MISSING_CONTEXT_FIELD");
    assert!(!diagnostics(&project)
        .iter()
        .any(|d| d.code == "FAIL_UNDECLARED_PROPAGATION"));
}

#[test]
fn exhaustive_statement_match_continues_only_if_an_arm_can_continue() {
    let source = r#"
enum Choice { left right }
type ResultText = Text {}
failure Missing { kind: NotFound code: "missing" }
function done(choice: Choice) -> ResultText {
    match choice {
        Choice.left => { return ResultText("left") }
        Choice.right => { return ResultText("right") }
    }
    reject Missing
}
"#;
    clean(source);
    let live = source.replace(
        "Choice.right => { return ResultText(\"right\") }",
        "Choice.right => { var value = ResultText(\"right\") }",
    );
    let project = analyze(&live);
    error(&project, "FAIL_UNDECLARED_PROPAGATION");
    clean(&live.replace("done(choice: Choice)", "done(choice: Choice) fails Missing"));
}

#[test]
fn rejection_terminates_before_later_rejections_and_calls() {
    let source = format!(
        r#"{PRELUDE}
function halt(flag: Flag) fails Missing -> ResultText {{
    reject Missing
    reject Refused
    return attempt lookup(flag)
}}
"#
    );
    let project = clean(&source);
    assert_eq!(failures(&project, "halt"), ["Missing"]);
    let stale = analyze(&source.replace(
        "halt(flag: Flag) fails Missing",
        "halt(flag: Flag) fails Missing, Refused",
    ));
    error(&stale, "FAIL_STALE_DECLARATION");
}

#[test]
fn unknown_outcome_has_no_semantic_http_default_but_bun_maps_explicitly() {
    let project = clean(
        r#"
output Result { ok: Bool }
failure Uncertain { kind: OutcomeUnknown code: "uncertain" }
action perform() fails Uncertain -> Result { reject Uncertain }
route POST /perform { auth: none output: Result run: perform() }
"#,
    );
    assert_eq!(project.failures.contracts[0].http_status, None);
    assert_eq!(project.failures.routes[0].http_status, None);
    let artifacts =
        jadpo_core::derive_artifacts(std::path::Path::new("validation.jadpo"), &project);
    let get = |name| {
        artifacts
            .iter()
            .find(|a| a.relative_path == name)
            .unwrap()
            .contents
            .as_str()
    };
    let manifest: serde_json::Value = serde_json::from_str(get("app.meta.json")).unwrap();
    assert_eq!(manifest["semantic_graph"]["schema_version"], 2);
    assert!(manifest["semantic_graph"]["failure_contracts"][0]["http_status"].is_null());
    let audit: serde_json::Value = serde_json::from_str(get("audit/failures.json")).unwrap();
    assert!(audit["failures"][0]["http_status"].is_null());
    let routes: serde_json::Value = serde_json::from_str(get("inventory/routes.json")).unwrap();
    assert_eq!(routes["routes"][0]["failures"][0]["http_status"], 500);
    let api: serde_json::Value = serde_json::from_str(get("openapi/openapi.json")).unwrap();
    assert!(api["paths"]["/perform"]["post"]["responses"]
        .get("500")
        .is_some());
}
