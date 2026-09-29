//! POLICY-D04–07, D12, D19, D22: role authority has one checked source,
//! scope is explicit when necessary, and exceptions match real entity effects.
use jadpo_core::analyze_sources;
use jadpo_diagnostics::Diagnostic;
use jadpo_syntax::SourceFile;

fn diagnostics(source: &str) -> Vec<Diagnostic> {
    let project = analyze_sources(vec![SourceFile::new(
        "policy-diagnostics.jadpo".into(),
        source.into(),
    )])
    .unwrap();
    project
        .syntax
        .diagnostics()
        .chain(project.semantics.diagnostics.iter())
        .chain(project.typing.diagnostics.iter())
        .chain(project.failures.diagnostics.iter())
        .cloned()
        .collect()
}
fn clean(source: &str) {
    assert!(diagnostics(source).is_empty(), "{:#?}", diagnostics(source));
}
fn reject(source: &str, code: &str, excerpt: Option<&str>) {
    let found = diagnostics(source);
    assert!(
        !found.iter().any(|d| d.code.starts_with("SYN_")),
        "unrelated parse failure: {found:#?}"
    );
    let diagnostic = found
        .iter()
        .find(|d| d.code == code)
        .unwrap_or_else(|| panic!("missing {code}: {found:#?}"));
    assert!(!diagnostic.message.is_empty());
    assert!(!diagnostic.recommended_next_step.title.is_empty());
    let span = diagnostic
        .primary
        .as_ref()
        .expect("authored policy needs a source location");
    assert_eq!(span.source, "policy-diagnostics.jadpo");
    assert!(span.start < span.end && span.end <= source.len());
    if let Some(excerpt) = excerpt {
        assert_eq!(&source[span.start..span.end], excerpt);
    }
    // A suggested repair must never manufacture a public grant.
    for edit in diagnostic.recommended_next_step.edits.iter().chain(
        diagnostic
            .alternatives
            .iter()
            .flat_map(|step| step.edits.iter()),
    ) {
        assert!(!edit.replacement.contains("Access.public"));
        assert!(!edit.replacement.contains("auth: none"));
    }
}
const DIRECT: &str = "enum NoteRole { owner }\nentity User { id: Uuid identity }\nentity Note { id: Uuid identity owner_id: User.id { role: NoteRole.owner immutable: true } policy { NoteRole.owner: [read] } }";
const MEMBERSHIP: &str = "enum AppRole { member }\nentity User { id: Uuid identity }\nentity Membership { id: Uuid identity user_id: User.id role: AppRole membership { scope: application member: user_id role: role } }";

#[test]
fn direct_role_binding_is_unique() {
    clean(DIRECT);
    reject(
        &DIRECT.replace(
            "role: NoteRole.owner",
            "role: NoteRole.owner role: NoteRole.owner",
        ),
        "POLICY_BINDING_DUPLICATE",
        Some("role: NoteRole.owner"),
    );
}
#[test]
fn explicit_mutability_cannot_change_authority() {
    reject(
        &DIRECT.replace("immutable: true", "immutable: false"),
        "POLICY_BINDING_MUTABLE",
        Some("false"),
    );
    clean(DIRECT);
}
#[test]
fn role_binding_requires_an_immutable_field() {
    reject(
        &DIRECT.replace(" immutable: true", ""),
        "POLICY_ROLE_FIELD_UPDATE_FORBIDDEN",
        Some("owner_id: User.id { role: NoteRole.owner }"),
    );
    clean(DIRECT);
}
#[test]
fn direct_binding_requires_a_qualified_declared_role_and_principal_reference() {
    reject(
        &DIRECT.replace("role: NoteRole.owner", "role: NoteRole.missing"),
        "POLICY_BINDING_INVALID",
        Some("role: NoteRole.missing"),
    );
    reject(
        &DIRECT.replace("owner_id: User.id", "owner_id: Text"),
        "POLICY_BINDING_INVALID",
        Some("owner_id: Text { role: NoteRole.owner immutable: true }"),
    );
    clean(DIRECT);
}
#[test]
fn entity_and_field_policy_each_have_one_authority() {
    reject(
        &DIRECT.replace(
            "policy { NoteRole.owner: [read] }",
            "policy { NoteRole.owner: [read] } policy { NoteRole.owner: [read] }",
        ),
        "POLICY_DECLARATION_DUPLICATE",
        None,
    );
    reject(
        &DIRECT.replace("immutable: true", "immutable: true policy {} policy {}"),
        "POLICY_DECLARATION_DUPLICATE",
        None,
    );
    clean(DIRECT);
}
#[test]
fn membership_has_one_declaration() {
    clean(MEMBERSHIP);
    reject(&MEMBERSHIP.replace("membership { scope: application member: user_id role: role }", "membership { scope: application member: user_id role: role } membership { scope: application member: user_id role: role }"), "POLICY_MEMBERSHIP_DUPLICATE", None);
}
#[test]
fn membership_requires_a_scope() {
    reject(
        &MEMBERSHIP.replace("scope: application ", ""),
        "POLICY_MEMBERSHIP_SCOPE_REQUIRED",
        Some("membership { member: user_id role: role }"),
    );
    clean(MEMBERSHIP);
}
#[test]
fn membership_requires_a_member() {
    reject(
        &MEMBERSHIP.replace("member: user_id ", ""),
        "POLICY_MEMBERSHIP_MEMBER_REQUIRED",
        Some("membership { scope: application role: role }"),
    );
    clean(MEMBERSHIP);
}
#[test]
fn membership_requires_a_role() {
    reject(
        &MEMBERSHIP.replace(" role: role", ""),
        "POLICY_MEMBERSHIP_ROLE_REQUIRED",
        Some("membership { scope: application member: user_id }"),
    );
    clean(MEMBERSHIP);
}
#[test]
fn explicit_scope_selection_cannot_be_repeated() {
    reject(
        &DIRECT.replace("policy {", "policy { scope: owner_id scope: owner_id"),
        "POLICY_SCOPE_DUPLICATE",
        Some("scope"),
    );
    clean(DIRECT);
}
#[test]
fn policy_effects_are_closed_and_not_repeated() {
    reject(
        &DIRECT.replace("[read]", "[read, read]"),
        "POLICY_EFFECT_DUPLICATE",
        Some("read"),
    );
    reject(
        &DIRECT.replace("[read]", "[publish]"),
        "POLICY_EFFECT_UNKNOWN",
        Some("publish"),
    );
    clean(DIRECT);
}
#[test]
fn role_declaration_does_not_itself_grant_authority() {
    reject(
        &DIRECT.replace(
            "owner_id: User.id { role: NoteRole.owner immutable: true } ",
            "",
        ),
        "POLICY_ROLE_UNBOUND",
        Some("policy { NoteRole.owner: [read] }"),
    );
    clean(DIRECT);
}
#[test]
fn protected_child_needs_a_relationship_to_its_role_scope() {
    let source = format!(
        "{DIRECT} entity Attachment {{ id: Uuid identity policy {{ NoteRole.owner: [read] }} }}"
    );
    reject(
        &source,
        "POLICY_SCOPE_MISSING",
        Some("policy { NoteRole.owner: [read] }"),
    );
    clean(&source.replace(
        "entity Attachment { id: Uuid identity",
        "entity Attachment { id: Uuid identity note_id: Note.id",
    ));
}
#[test]
fn subject_is_canonical_in_one_rule() {
    reject(
        &DIRECT.replace(
            "NoteRole.owner: [read]",
            "NoteRole.owner: [read] NoteRole.owner: [update]",
        ),
        "POLICY_SUBJECT_DUPLICATE",
        Some("NoteRole.owner: [update]"),
    );
    clean(DIRECT);
}
#[test]
fn exception_names_an_existing_entity_operation() {
    reject(
        &DIRECT.replace(
            "policy {",
            "policy { operations { missing { NoteRole.owner: [read] } }",
        ),
        "POLICY_OPERATION_UNKNOWN",
        Some("missing"),
    );
    clean(DIRECT);
}
#[test]
fn exception_must_match_the_entity_effects_its_operation_reaches() {
    let source = format!("type Label = Text {{}} {}", DIRECT.replace("policy {", "function label() -> Label { return Label(\"label\") } policy { operations { label { NoteRole.owner: [read] } }"));
    reject(
        &source,
        "POLICY_OPERATION_EFFECT_MISMATCH",
        Some("NoteRole.owner: [read]"),
    );
    clean(&source.replace("operations { label { NoteRole.owner: [read] } }", ""));
}
