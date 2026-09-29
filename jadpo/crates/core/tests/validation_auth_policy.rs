//! AUTH-001 §1–2 and POLICY-001 D12/D15/D16: no default public route,
//! authentication boundary data cannot escape via a container, and policy
//! weakening is not an automatic diagnostic repair.
use jadpo_core::{analyze_sources, derive_target, AnalyzedProject};
use jadpo_diagnostics::Diagnostic;
use jadpo_syntax::SourceFile;
use std::path::Path;

const BASE: &str = include_str!("../../../../examples/first-party-authentication/app.jadpo");
fn analyze(source: &str) -> AnalyzedProject {
    analyze_sources(vec![SourceFile::new(
        "auth-policy.jadpo".into(),
        source.into(),
    )])
    .unwrap()
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
        "{:#?}",
        diagnostics(&project)
    );
    project
}
fn require<'a>(project: &'a AnalyzedProject, code: &str) -> &'a Diagnostic {
    diagnostics(project)
        .into_iter()
        .find(|d| d.code == code)
        .unwrap_or_else(|| panic!("missing {code}: {:#?}", diagnostics(project)))
}
fn safe_repair(diagnostic: &Diagnostic) {
    assert!(!diagnostic.message.is_empty());
    assert!(!diagnostic.recommended_next_step.title.is_empty());
    for edit in diagnostic.recommended_next_step.edits.iter().chain(
        diagnostic
            .alternatives
            .iter()
            .flat_map(|step| step.edits.iter()),
    ) {
        assert!(
            !edit.replacement.contains("auth: none"),
            "diagnostic must not silently deauthenticate a route"
        );
        assert!(
            !edit.replacement.contains("Access.public"),
            "diagnostic must not silently grant public access"
        );
    }
}

#[test]
fn protected_route_without_adapter_configuration_remains_unsupported() {
    let source = BASE.replace("            secret: config.signing_key\n", "");
    let project = clean(&source);
    let diagnostic = derive_target(Path::new("auth-policy.jadpo"), &project)
        .expect_err("unconfigured protected runtime must fail closed");
    assert_eq!(diagnostic.code, "JADPO_TARGET_AUTH_NOT_IMPLEMENTED");
    safe_repair(&diagnostic);
    derive_target(Path::new("auth-policy.jadpo"), &clean(BASE))
        .expect("configured supported source generates");
}

#[test]
fn immediate_revocation_cannot_be_claimed_by_a_signed_only_adapter() {
    let immediate = BASE.replace(
        "mode: bounded\n            maximum_delay: 5m",
        "mode: immediate",
    );
    let project = clean(&immediate);
    let diagnostic = derive_target(Path::new("auth-policy.jadpo"), &project)
        .expect_err("signed adapter does not implement immediate revocation");
    assert_eq!(diagnostic.code, "JADPO_TARGET_AUTH_NOT_IMPLEMENTED");
    safe_repair(&diagnostic);
    let opaque = immediate.replace("mode: signed", "mode: opaque");
    derive_target(Path::new("auth-policy.jadpo"), &clean(&opaque))
        .expect("opaque authority-checked adapters support immediate mode");
}

#[test]
fn bounded_revocation_requires_an_explicit_delay_with_no_public_access_repair() {
    let source = BASE.replace("            maximum_delay: 5m\n", "");
    let project = analyze(&source);
    let diagnostic = require(&project, "SEM_REVOCATION_DELAY_REQUIRED");
    let span = diagnostic.primary.as_ref().unwrap();
    assert!(source[span.start..span.end].starts_with("revocation {"));
    safe_repair(diagnostic);
    clean(BASE);
}

#[test]
fn public_routes_cannot_hide_principal_access_behind_a_transitive_query() {
    let source = format!(
        r#"{BASE}
query wrapper() -> CurrentIdentity {{ return current_identity() }}
route GET /wrapped {{ auth: none output: CurrentIdentity run: wrapper() }}
"#
    );
    let project = analyze(&source);
    let diagnostic = require(&project, "TYPE_AUTH_PUBLIC_PRINCIPAL");
    let span = diagnostic.primary.as_ref().unwrap();
    assert!(source[span.start..span.end].contains("/wrapped"));
    safe_repair(diagnostic);
    clean(&source.replace("/wrapped { auth: none", "/wrapped {"));
}

#[test]
fn principal_variants_cannot_escape_nested_public_output_containers() {
    for field_type in [
        "Principal.user",
        "List<Principal.user>",
        "Map<Text, Principal.user>",
        "Wrapped",
    ] {
        let source = format!("{BASE}\nvalue Wrapped {{ actor: Principal.user }}\noutput Leak {{ actor: {field_type} }}");
        let project = analyze(&source);
        let diagnostic = require(&project, "TYPE_AUTH_PRINCIPAL_OUTPUT");
        let span = diagnostic.primary.as_ref().unwrap();
        assert_eq!(&source[span.start..span.end], field_type);
        safe_repair(diagnostic);
    }
}

#[test]
fn neither_public_nor_internal_failure_context_is_a_principal_sink() {
    for channel in ["public", "internal"] {
        let source = format!("{BASE}\nfailure Leak {{ kind: Rejected code: \"leak\" {channel} {{ actor: List<Principal.user> }} }}");
        let project = analyze(&source);
        let diagnostic = require(&project, "TYPE_AUTH_PRINCIPAL_FAILURE_CONTEXT");
        let span = diagnostic.primary.as_ref().unwrap();
        assert_eq!(&source[span.start..span.end], "List<Principal.user>");
        safe_repair(diagnostic);
    }
}

#[test]
fn a_narrowing_field_cannot_grant_an_entity_effect_that_was_never_allowed() {
    let source = r#"
enum NoteRole { owner }
entity User { id: Uuid identity }
entity Note {
    id: Uuid identity
    owner_id: User.id { role: NoteRole.owner immutable: true }
    title: Text { policy { NoteRole.owner: [read, update] } }
    policy { NoteRole.owner: [read] }
}
"#;
    let project = analyze(source);
    let diagnostic = require(&project, "POLICY_FIELD_WIDENS_ENTITY");
    safe_repair(diagnostic);
    // Repair by narrowing the field's grant, not widening the human-owned
    // entity permission. No new authority is introduced by this repair.
    clean(&source.replace("NoteRole.owner: [read, update]", "NoteRole.owner: [read]"));
}
