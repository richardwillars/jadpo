//! AUTH-P5/P6: capability settings cannot introduce raw keys, package choices,
//! algorithms, optional ownership or an unrelated authority binding.
use jadpo_core::analyze_sources;
use jadpo_syntax::SourceFile;

const BASE: &str =
    include_str!("../../../../tests/compile/pass/128_authentication_strategy_topology.jadpo");
fn check(source: &str) -> Vec<String> {
    let project = analyze_sources(vec![SourceFile::new(
        "auth-settings.jadpo".into(),
        source.into(),
    )])
    .unwrap();
    project
        .syntax
        .diagnostics()
        .chain(project.semantics.diagnostics.iter())
        .chain(project.typing.diagnostics.iter())
        .chain(project.failures.diagnostics.iter())
        .map(|d| format!("{}: {}", d.code, d.message))
        .collect()
}
fn jwt(settings: &str) -> String {
    BASE.replace(
        "mode: jwt\n            principal: user",
        &format!("mode: jwt\n            principal: user\n            {settings}"),
    )
}
fn owner(binding: &str, declaration: &str) -> String {
    BASE.replace(
        "entity Service {",
        &format!("entity Service {{\n {declaration}"),
    )
    .replace(
        "mode: api_key\n            principal: service",
        &format!("mode: api_key\n            principal: service\n owner: {binding}"),
    )
}
#[test]
fn inactive_principals_preserve_declared_permission_or_domain_failure() {
    for kind in ["NotPermitted", "Rejected"] {
        assert_eq!(check(&BASE.replace("kind: Rejected", &format!("kind: {kind}"))), Vec::<String>::new());
    }
    for kind in ["NotFound", "Conflict", "Internal"] {
        assert!(check(&BASE.replace("kind: Rejected", &format!("kind: {kind}")))
            .iter().any(|d| d.starts_with("TYPE_AUTH_INACTIVE_FAILURE_KIND")), "{kind}");
    }
}

#[test]
fn jwt_accepts_only_nonsecret_endpoint_and_audience_configuration() {
    let source = jwt("issuer: \"https://issuer.example\" audience: \"jadpo\" jwks_uri: \"https://keys.example/jwks\"");
    assert_eq!(check(&source), Vec::<String>::new());
    let configured = format!(
        "config JwtConfig {{ issuer: Url {{ binding: \"ISSUER\" }} }}\n{}",
        jwt("issuer: config.issuer audience: \"jadpo\"")
    );
    assert_eq!(check(&configured), Vec::<String>::new());
    for setting in [
        "issuer: 42",
        "issuer: none",
        "algorithms: \"RS256\"",
        "package: \"jose\"",
        "secret: \"hidden\"",
        "origin: \"https://app.example\"",
        "audience: \"one\" audience: \"two\"",
    ] {
        assert!(
            check(&jwt(setting))
                .iter()
                .any(|d| d.starts_with("TYPE_AUTH_ADAPTER_SETTING")),
            "{setting}"
        );
    }
    let secret = format!(
        "config JwtConfig {{ issuer: Url {{ binding: \"ISSUER\" secret: true }} }}\n{}",
        jwt("issuer: config.issuer")
    );
    assert!(check(&secret)
        .iter()
        .any(|d| d.starts_with("TYPE_AUTH_ADAPTER_SETTING")));
}
#[test]
fn endpoint_settings_cannot_be_added_to_non_jwt_validators() {
    for mode in ["signed", "opaque", "api_key"] {
        let source = jwt("issuer: \"https://issuer.example\"")
            .replace("mode: jwt", &format!("mode: {mode}"));
        assert!(
            check(&source)
                .iter()
                .any(|d| d.starts_with("TYPE_AUTH_ADAPTER_SETTING")),
            "{mode}"
        );
    }
}
#[test]
fn service_owner_is_explicit_required_persistent_reference() {
    let declaration = "owner_id: User.id references User.id on_delete restrict";
    let source = owner("Service.owner_id", declaration);
    assert_eq!(check(&source), Vec::<String>::new());
    for (binding, declaration) in [
        ("Service.owner_id", "owner_id: Uuid"),
        (
            "Service.owner_id",
            "owner_id: User.id? references User.id on_delete set_null",
        ),
        ("User.id", declaration),
        ("Service.missing", declaration),
        ("\"User.id\"", declaration),
    ] {
        let diagnostics = check(&owner(binding, declaration));
        assert!(
            diagnostics
                .iter()
                .any(|d| d.starts_with("TYPE_AUTH_ADAPTER_SETTING")),
            "{binding}: {diagnostics:?}"
        );
    }
}
