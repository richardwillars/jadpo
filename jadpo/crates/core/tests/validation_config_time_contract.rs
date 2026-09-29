//! Contract-derived CONFIG-D05/D09, TIME-D04 and TEST-D07 boundary cases.
use jadpo_core::analyze_sources;
use jadpo_diagnostics::Diagnostic;
use jadpo_syntax::SourceFile;

fn diagnostics(source: &str) -> Vec<Diagnostic> {
    let project = analyze_sources(vec![SourceFile::new(
        "config-time.jadpo".into(),
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
fn reject(source: &str, code: &str) {
    let found = diagnostics(source);
    assert!(
        !found.iter().any(|d| d.code.starts_with("SYN_")),
        "test must reach semantics: {found:#?}"
    );
    let diagnostic = found
        .iter()
        .find(|d| d.code == code)
        .unwrap_or_else(|| panic!("missing {code}: {source}\n{found:#?}"));
    assert!(!diagnostic.message.is_empty());
    assert!(!diagnostic.recommended_next_step.title.is_empty());
    let span = diagnostic
        .primary
        .as_ref()
        .expect("authored boundary needs a span");
    assert!(source.get(span.start..span.end).is_some());
}
fn reject_at(source: &str, code: &str, excerpt: &str) {
    reject(source, code);
    let found = diagnostics(source);
    let diagnostic = found.iter().find(|d| d.code == code).unwrap();
    let span = diagnostic.primary.as_ref().unwrap();
    assert_eq!(&source[span.start..span.end], excerpt);
}
const SECRET: &str = "type ApiKey = Text { min_length: 8 }\nconfig Settings { api_key: ApiKey { binding: \"WAVE_KEY\" secret: true } }";

#[test]
fn secret_classification_survives_local_aliases() {
    reject(&format!("{SECRET}\nfunction leak() -> ApiKey {{ var first = config.api_key var second = first return second }}"), "CONFIG_SECRET_FLOW");
}
#[test]
fn explicit_construction_cannot_remove_secret_classification() {
    for expression in ["ApiKey(alias)", "ApiKey(ApiKey(alias))"] {
        reject_at(&format!("{SECRET}\nfunction leak() -> ApiKey {{ var alias = config.api_key return {expression} }}"), "CONFIG_SECRET_FLOW", expression);
    }
}
#[test]
fn temporal_helpers_are_not_secret_sinks() {
    reject_at("type PrivateInstant = Instant {}\nconfig Settings { private_time: PrivateInstant { binding: \"PRIVATE_TIME\" secret: true } }\ntype ResultTime = Object { value: Time }\nfunction leak() -> ResultTime { return ResultTime { value: temporal.in_zone(config.private_time, Zone.europe_london) } }", "CONFIG_SECRET_FLOW", "config.private_time");
}
#[test]
fn secret_values_cannot_cross_record_or_callable_boundaries() {
    for source in [
        "type Envelope = Object { key: ApiKey }\nfunction leak() -> Envelope { return Envelope { key: config.api_key } }",
        "function echo_key(key: ApiKey) -> ApiKey { return key }\nfunction leak() -> ApiKey { return echo_key(config.api_key) }",
    ] { reject(&format!("{SECRET}\n{source}"), "CONFIG_SECRET_FLOW"); }
}
#[test]
fn secret_comparisons_cannot_disclose_information_as_a_boolean() {
    reject(&format!("{SECRET}\ntype Flag = Bool {{}}\nfunction leak() -> Flag {{ return Flag(config.api_key == config.api_key) }}"), "CONFIG_SECRET_FLOW");
}
#[test]
fn secret_defaults_are_rejected_even_when_nominally_valid() {
    reject("type ApiKey = Text { min_length: 8 }\nconfig Settings { api_key: ApiKey { binding: \"WAVE_KEY\" secret: true default: \"fixture-canary-value\" } }", "CONFIG_SECRET_DEFAULT");
}
#[test]
fn fixture_secret_wrappers_are_required_and_cannot_mark_public_configuration() {
    reject(
        &format!(
            "{SECRET}\nfixture raw_secret {{ config {{ api_key: \"fixture-canary-value\" }} }}"
        ),
        "CONFIG_FIXTURE_SECRET_REQUIRED",
    );
    reject("type Label = Text {}\nconfig Settings { label: Label { binding: \"WAVE_LABEL\" } }\nfixture secret_to_public { config { label: secret(\"fixture-canary-value\") } }", "CONFIG_FIXTURE_SECRET_UNEXPECTED");
}
#[test]
fn fixture_secret_values_still_obey_nominal_constraints() {
    reject(
        &format!("{SECRET}\nfixture invalid_secret {{ config {{ api_key: secret(\"short\") }} }}"),
        "CONFIG_FIXTURE_VALUE_INVALID",
    );
}
#[test]
fn pure_callables_receive_time_instead_of_reading_the_clock() {
    reject(
        "function read_time() -> Instant { return clock.now }",
        "TYPE_CLOCK_CONTEXT",
    );
    let accepted = diagnostics("type ClockReading = Object { now: Instant }\nfunction echo_time(reading: ClockReading) -> ClockReading { return reading }");
    assert!(accepted.is_empty(), "{accepted:#?}");
}
#[test]
fn production_actions_cannot_advance_the_test_clock() {
    reject(
        "action read_time() -> Instant { advance clock by Duration(\"PT1S\") return clock.now }",
        "TYPE_TEST_CLOCK_CONTEXT",
    );
}
#[test]
fn temporal_literal_calendar_precision_and_offset_boundaries_are_checked() {
    for literal in [
        "2025-02-29T12:00:00Z",
        "2026-04-31T12:00:00Z",
        "2026-01-01T24:00:00Z",
        "2026-01-01T12:00:00",
        "2026-01-01T12:00:00.0001Z",
        "2026-01-01T12:00:00-00:00",
        "0001-01-01T00:00:00+01:00",
        "0001-01-01T00:59:59.999+01:00",
        "9999-12-31T23:59:59-01:00",
    ] {
        reject(&format!("type ClockReading = Object {{ now: Instant }}\nfunction invalid_time() -> ClockReading {{ return ClockReading {{ now: Instant(\"{literal}\") }} }}"), "TYPE_INVALID_LITERAL");
    }
    for literal in [
        "2024-02-29T12:00:00Z",
        "2026-01-01T23:59:59.999Z",
        "2026-01-01T12:00:00+05:45",
        "0001-01-01T01:00:00+01:00",
        "9999-12-31T22:59:59.999-01:00",
    ] {
        let found = diagnostics(&format!("type ClockReading = Object {{ now: Instant }}\nfunction valid_time() -> ClockReading {{ return ClockReading {{ now: Instant(\"{literal}\") }} }}"));
        assert!(found.is_empty(), "{literal}: {found:#?}");
    }
}
