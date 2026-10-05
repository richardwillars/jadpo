//! Source-level credential lifecycle binding is explicit and independent of names.
use jadpo_core::{analyze_sources, derive_artifacts, format_source};
use jadpo_syntax::SourceFile;
use std::path::Path;

const BASE: &str =
    include_str!("../../../../tests/compile/pass/128_authentication_strategy_topology.jadpo");
const BINDING: &str = r#"credentials {
    identity: AccessGrant.key
    principal: AccessGrant.worker
    verifier: AccessGrant.digest
    active: state == GrantState.usable
    expires: AccessGrant.deadline
    revoked: AccessGrant.withdrawn
}"#;
fn source() -> String {
    format!(
        r#"
enum GrantState {{ usable retired }}
entity AccessGrant {{
    key: Uuid identity
    worker: Service.id references Service.id on_delete restrict
    digest: Text unique
    state: GrantState
    deadline: Instant
    withdrawn: Instant?
}}
{}"#,
        BASE.replace(
            "mode: api_key\n            principal: service",
            &format!("mode: api_key\n principal: service\n {BINDING}")
        )
    )
}
fn diagnostics(source: &str) -> Vec<String> {
    let project =
        analyze_sources(vec![SourceFile::new("binding.jadpo".into(), source.into())]).unwrap();
    project
        .syntax
        .diagnostics()
        .chain(project.semantics.diagnostics.iter())
        .chain(project.typing.diagnostics.iter())
        .chain(project.failures.diagnostics.iter())
        .map(|d| d.code.to_string())
        .collect()
}
fn rejected(source: String) {
    let errors = diagnostics(&source);
    assert!(
        !errors.is_empty(),
        "accepted invalid credential binding: {source}"
    );
}
#[test]
fn renamed_binding_roundtrips_and_exports_source_contract() {
    for source in [
        source(),
        source()
            .replace("AccessGrant", "RobotTicket")
            .replace("Service", "Worker")
            .replace("digest", "fingerprint")
            .replace("deadline", "end_at")
            .replace("withdrawn", "disabled_at"),
    ] {
        assert_eq!(diagnostics(&source), Vec::<String>::new());
        let formatted = format_source(&source);
        assert_eq!(diagnostics(&formatted), Vec::<String>::new());
        assert_eq!(formatted, format_source(&formatted));
        let project =
            analyze_sources(vec![SourceFile::new("binding.jadpo".into(), formatted)]).unwrap();
        let artifacts = derive_artifacts(Path::new("."), &project);
        let audit = &artifacts
            .iter()
            .find(|a| a.relative_path == "audit/authentication.json")
            .unwrap()
            .contents;
        assert!(
            audit.contains("\"active\":\"state == GrantState.usable\""),
            "{audit}"
        );
        assert!(audit.contains("\"credentials\":{\"identity\":"), "{audit}");
        assert!(project
            .semantics
            .nodes
            .iter()
            .any(|n| n.name.contains("credentials.verifier.")));
    }
}
#[test]
fn binding_is_only_available_on_service_api_keys() {
    for mode in ["signed", "opaque", "jwt"] {
        rejected(source().replace("mode: api_key", &format!("mode: {mode}")));
    }
    rejected(source().replace(
        "mode: api_key\n principal: service",
        "mode: api_key\n principal: user",
    ));
}
#[test]
fn binding_requires_exactly_one_of_each_known_role() {
    for line in BINDING.lines().skip(1).take(6) {
        rejected(source().replace(line, ""));
    }
    for line in BINDING.lines().skip(1).take(6) {
        rejected(source().replace(line, &format!("{line}\n{line}")));
    }
    rejected(source().replace(BINDING, &format!("{BINDING}\n{BINDING}")));
    rejected(source().replace("identity: AccessGrant.key", "unknown: AccessGrant.key"));
}
#[test]
fn credential_field_types_references_and_roles_fail_closed() {
    for (from, to) in [
        ("key: Uuid identity", "key: Text identity"),
        ("key: Uuid identity", "key: Uuid unique"),
        (
            "worker: Service.id references Service.id on_delete restrict",
            "worker: User.id references User.id on_delete restrict",
        ),
        (
            "worker: Service.id references Service.id on_delete restrict",
            "worker: Service.id? references Service.id on_delete set_null",
        ),
        (
            "worker: Service.id references Service.id on_delete restrict",
            "worker: Uuid",
        ),
        ("digest: Text unique", "digest: Text"),
        ("digest: Text unique", "digest: Uuid unique"),
        ("deadline: Instant", "deadline: Instant?"),
        ("withdrawn: Instant?", "withdrawn: Instant"),
        ("expires: AccessGrant.deadline", "expires: Service.deadline"),
        ("verifier: AccessGrant.digest", "verifier: AccessGrant.key"),
        (
            "principal: AccessGrant.worker",
            "principal: Principal.service.service_id",
        ),
        (
            "verifier: AccessGrant.digest",
            "verifier: AccessGrant.missing",
        ),
    ] {
        rejected(source().replace(from, to));
    }
}
#[test]
fn active_predicate_must_be_constructible_and_cannot_alias_identity_fields() {
    for active in [
        "true",
        "state != GrantState.retired",
        "state == GrantState.usable or state == GrantState.retired",
        "state == GrantState.missing",
        "key == GrantState.usable",
        "principal == true",
        "state == Service.enabled",
    ] {
        rejected(source().replace("state == GrantState.usable", active));
    }
    let boolean = source()
        .replace("state: GrantState", "state: Bool")
        .replace("state == GrantState.usable", "state == true");
    assert_eq!(diagnostics(&boolean), Vec::<String>::new());
}

#[test]
fn credential_authority_uses_same_store_and_service_identity() {
    let dossier = source().split("output PrincipalIdentity").next().unwrap().to_owned()
        .replace("key: Uuid identity", "key: Uuid")
        .replace("digest: Text unique", "digest: Text")
        .replace("withdrawn: Instant?", "withdrawn: Instant?\n identity: key\n persistence { store: primary role: authority unique: digest }")
        .replace("id: Uuid identity\n    credential_subject: Text unique\n    enabled: Bool", "id: Uuid\n credential_subject: Text\n enabled: Bool\n identity: id\n persistence { store: primary role: authority unique: credential_subject }");
    assert_eq!(diagnostics(&dossier), Vec::<String>::new());
    rejected(dossier.replacen("store: primary", "store: other", 1));
    rejected(source().replace(
        "worker: Service.id references Service.id",
        "worker: Service.credential_subject references Service.credential_subject",
    ));
    for field in [
        "key: Uuid identity",
        "worker: Service.id references Service.id on_delete restrict",
        "digest: Text unique",
        "deadline: Instant",
        "withdrawn: Instant?",
        "state: GrantState",
    ] {
        rejected(source().replace(field, &format!("{field} optional")));
    }
}

#[test]
fn lifecycle_generated_fields_cannot_replace_credential_roles() {
    for field in ["deadline: Instant", "withdrawn: Instant?"] {
        let errors =
            diagnostics(&source().replace(field, &format!("{field} generated {{ on: create }}")));
        assert!(
            errors
                .iter()
                .any(|code| code == "TYPE_AUTH_ADAPTER_SETTING"),
            "{errors:?}"
        );
        assert!(
            !errors.iter().any(|code| code.starts_with("SYN_")),
            "{errors:?}"
        );
    }
    let generated_identity = source().replace(
        "key: Uuid identity",
        "key: Uuid { generated: identity } identity",
    );
    assert_eq!(diagnostics(&generated_identity), Vec::<String>::new());
}
