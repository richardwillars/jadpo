//! Deterministic filesystem failure boundaries; only disposable directories are changed.
use jadpo_core::{analyze_sources, check_local_configuration, set_local_configuration};
use jadpo_syntax::SourceFile;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "jadpo-local-config-validation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn project() -> jadpo_core::AnalyzedProject {
    analyze_sources(vec![SourceFile::new(
        "app.jadpo".into(),
        r#"type ApiKey = Text { min_length: 3 }
config Settings { api_key: ApiKey { binding: "API_KEY" secret: true } }"#
            .into(),
    )])
    .unwrap()
}
#[test]
fn invalid_encoding_is_not_overwritten_or_reported_as_a_value() {
    let root = Fixture::new();
    let path = root.0.join(".env.local");
    fs::write(&path, [0xff, 0xfe]).unwrap();
    let error =
        set_local_configuration(&root.0, &project(), "api_key", "valid-secret").unwrap_err();
    assert_eq!(error.code, "CONFIG_LOCAL_INVALID_ENCODING");
    assert_eq!(fs::read(&path).unwrap(), [0xff, 0xfe]);
    assert_eq!(
        check_local_configuration(&root.0, &project())
            .unwrap_err()
            .code,
        "CONFIG_LOCAL_READ_FAILED"
    );
    assert!(!format!("{error:?}").contains("valid-secret"));
}
#[test]
fn duplicate_or_malformed_assignments_are_rejected() {
    let root = Fixture::new();
    for contents in [
        "API_KEY=one\nAPI_KEY=two\n",
        "not-an-assignment",
        "API_KEY=\"unterminated",
    ] {
        fs::write(root.0.join(".env.local"), contents).unwrap();
        assert_eq!(
            check_local_configuration(&root.0, &project())
                .unwrap_err()
                .code,
            "CONFIG_LOCAL_SYNTAX"
        );
        assert_eq!(
            fs::read_to_string(root.0.join(".env.local")).unwrap(),
            contents
        );
    }
}
#[test]
fn non_file_configuration_target_is_rejected() {
    let root = Fixture::new();
    fs::create_dir(root.0.join(".env.local")).unwrap();
    assert_eq!(
        set_local_configuration(&root.0, &project(), "api_key", "valid-secret")
            .unwrap_err()
            .code,
        "CONFIG_LOCAL_UNSAFE_TARGET"
    );
    assert!(root.0.join(".env.local").is_dir());
}
#[test]
fn atomic_write_collision_preserves_original_and_foreign_temporary_file() {
    let root = Fixture::new();
    let path = root.0.join(".env.local");
    fs::write(&path, "API_KEY=original-secret\n").unwrap();
    let temporary = root
        .0
        .join(format!(".env.local.jadpo.{}.tmp", std::process::id()));
    fs::write(&temporary, "concurrent-writer").unwrap();
    assert_eq!(
        set_local_configuration(&root.0, &project(), "api_key", "replacement-secret")
            .unwrap_err()
            .code,
        "CONFIG_LOCAL_WRITE_FAILED"
    );
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "API_KEY=original-secret\n"
    );
    assert_eq!(fs::read_to_string(&temporary).unwrap(), "concurrent-writer");
}
