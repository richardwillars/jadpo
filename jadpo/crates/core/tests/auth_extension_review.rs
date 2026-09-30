//! Independent AUTH-P5/P6 review regressions derived from the accepted contract.
use jadpo_core::analyze_sources;
use jadpo_syntax::SourceFile;

const BASE: &str =
    include_str!("../../../../tests/compile/pass/128_authentication_strategy_topology.jadpo");

fn diagnostics(source: String) -> Vec<String> {
    let project = analyze_sources(vec![SourceFile::new(
        "auth-extension-review.jadpo".into(),
        source,
    )])
    .unwrap();
    project
        .syntax
        .diagnostics()
        .chain(project.semantics.diagnostics.iter())
        .chain(project.typing.diagnostics.iter())
        .chain(project.failures.diagnostics.iter())
        .map(|diagnostic| diagnostic.code.to_string())
        .collect()
}

#[test]
fn accepted_jwt_bearer_topology_remains_valid() {
    assert_eq!(diagnostics(BASE.to_owned()), Vec::<String>::new());
}

#[test]
fn jwt_cookie_transport_is_rejected_even_when_its_slot_is_unique() {
    // The prior selector test rejected a cookie only because its strategy had
    // no JWT validator. This source actually declares JWT in that cookie slot.
    let source = BASE.replace(
        "bearer: authorization_header",
        "cookie: \"external_jwt_cookie\"",
    );
    let errors = diagnostics(source);
    assert!(
        errors.iter().any(|code| code == "TYPE_AUTH_JWT_TRANSPORT"),
        "JWT is an explicitly bearer-only capability: {errors:?}"
    );
}

fn service_owner(validator: &str) -> String {
    BASE.replace(
        "entity Service {",
        "entity Service {\n owner_id: User.id references User.id on_delete restrict",
    )
    .replace("mode: api_key\n            principal: service", validator)
}

#[test]
fn owner_setting_is_rejected_outside_service_api_key_mode() {
    for validator in [
        "mode: signed principal: service owner: Service.owner_id",
        "mode: opaque principal: service owner: Service.owner_id",
        "mode: jwt principal: service owner: Service.owner_id",
        "mode: api_key principal: user owner: Service.owner_id",
    ] {
        let errors = diagnostics(service_owner(validator));
        assert!(
            errors
                .iter()
                .any(|code| code == "TYPE_AUTH_ADAPTER_SETTING"),
            "{validator}: {errors:?}"
        );
    }
}

#[test]
fn duplicate_owner_setting_is_not_silently_selected() {
    let errors = diagnostics(service_owner(
        "mode: api_key principal: service owner: Service.owner_id owner: Service.owner_id",
    ));
    assert!(
        errors
            .iter()
            .any(|code| code == "TYPE_AUTH_ADAPTER_SETTING"),
        "{errors:?}"
    );
}
