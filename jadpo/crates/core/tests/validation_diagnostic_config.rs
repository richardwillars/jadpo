//! CONFIG-D04/D09 and TEST-D05/D07: declared configuration is closed, required,
//! validated before use, and supplied explicitly in isolated authored fixtures.
use jadpo_core::{
    analyze_sources, local_configuration_environment, set_local_configuration, AnalyzedProject,
};
use jadpo_diagnostics::Diagnostic;
use jadpo_syntax::SourceFile;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
const CONFIG: &str = "type Label = Text { min_length: 3 } config Settings { label: Label { binding: \"DIAGNOSTIC_LABEL\" } }";
fn analyze(source: &str) -> AnalyzedProject {
    analyze_sources(vec![SourceFile::new(
        "configuration-diagnostics.jadpo".into(),
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
fn reject(source: &str, code: &str, excerpt: &str) {
    let project = analyze(source);
    let found = diagnostics(&project);
    assert!(
        !found.iter().any(|d| d.code.starts_with("SYN_")),
        "{found:#?}"
    );
    let diagnostic = found
        .iter()
        .find(|d| d.code == code)
        .unwrap_or_else(|| panic!("missing {code}: {found:#?}"));
    explain(diagnostic);
    let span = diagnostic.primary.as_ref().unwrap();
    assert_eq!(span.source, "configuration-diagnostics.jadpo");
    assert_eq!(&source[span.start..span.end], excerpt);
}
fn explain(diagnostic: &Diagnostic) {
    assert!(!diagnostic.message.is_empty());
    assert!(!diagnostic.recommended_next_step.title.is_empty());
}
struct Local(PathBuf);
impl Local {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "jadpo-diagnostic-config-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Local {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn fixture_cannot_invent_configuration_fields() {
    reject(
        &format!("{CONFIG} fixture local {{ config {{ label: \"valid\" typo: \"valid\" }} }}"),
        "CONFIG_FIELD_UNKNOWN",
        "typo",
    );
    clean(&format!(
        "{CONFIG} fixture local {{ config {{ label: \"valid\" }} }}"
    ));
}
#[test]
fn fixture_cannot_supply_two_competing_values() {
    reject(
        &format!("{CONFIG} fixture local {{ config {{ label: \"first\" label: \"second\" }} }}"),
        "CONFIG_FIXTURE_DUPLICATE",
        "label",
    );
    clean(&format!(
        "{CONFIG} fixture local {{ config {{ label: \"first\" }} }}"
    ));
}
#[test]
fn fixture_requires_each_non_defaulted_configuration_value() {
    reject(
        &format!("{CONFIG} fixture local {{ config {{ }} }}"),
        "CONFIG_FIXTURE_VALUE_MISSING",
        "fixture local { config { } }",
    );
    clean(&format!(
        "{} fixture local {{ config {{ }} }}",
        CONFIG.replace(
            "binding: \"DIAGNOSTIC_LABEL\"",
            "binding: \"DIAGNOSTIC_LABEL\" default: \"valid\""
        )
    ));
}
#[test]
fn local_set_rejects_unknown_field_without_creating_a_file() {
    let local = Local::new();
    let project = clean(CONFIG);
    let diagnostic = set_local_configuration(&local.0, &project, "typo", "valid").unwrap_err();
    assert_eq!(diagnostic.code, "CONFIG_FIELD_UNKNOWN");
    explain(&diagnostic);
    assert!(!local.0.join(".env.local").exists());
    set_local_configuration(&local.0, &project, "label", "valid").unwrap();
    assert_eq!(
        local_configuration_environment(&local.0, &project).unwrap()["DIAGNOSTIC_LABEL"],
        "valid"
    );
}
#[test]
fn local_environment_requires_a_value_or_checked_default() {
    let local = Local::new();
    let project = clean(CONFIG);
    let diagnostic = local_configuration_environment(&local.0, &project).unwrap_err();
    assert_eq!(diagnostic.code, "CONFIG_VALUE_MISSING");
    explain(&diagnostic);
    assert!(!local.0.join(".env.local").exists());
    let defaulted = clean(&CONFIG.replace(
        "binding: \"DIAGNOSTIC_LABEL\"",
        "binding: \"DIAGNOSTIC_LABEL\" default: \"valid\"",
    ));
    assert!(local_configuration_environment(&local.0, &defaulted)
        .unwrap()
        .is_empty());
}
#[test]
fn invalid_local_values_cannot_replace_valid_values_or_reach_runtime() {
    let local = Local::new();
    let project = clean(CONFIG);
    set_local_configuration(&local.0, &project, "label", "valid").unwrap();
    let path = local.0.join(".env.local");
    let original = fs::read(&path).unwrap();
    for invalid in ["xy", "bad\nINJECTED=value", "bad\rvalue", "bad\0value"] {
        let diagnostic = set_local_configuration(&local.0, &project, "label", invalid).unwrap_err();
        assert_eq!(diagnostic.code, "CONFIG_VALUE_INVALID");
        explain(&diagnostic);
        assert_eq!(fs::read(&path).unwrap(), original);
    }
    fs::write(&path, "DIAGNOSTIC_LABEL=xy\n").unwrap();
    let diagnostic = local_configuration_environment(&local.0, &project).unwrap_err();
    assert_eq!(diagnostic.code, "CONFIG_VALUE_INVALID");
    explain(&diagnostic);
    set_local_configuration(&local.0, &project, "label", "repaired").unwrap();
    assert_eq!(
        local_configuration_environment(&local.0, &project).unwrap()["DIAGNOSTIC_LABEL"],
        "repaired"
    );
}
