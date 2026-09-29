//! Test-command process outcomes use real CLI builds and controlled disposable launchers.
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new(source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "jadpo-test-command-validation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("bin")).unwrap();
        fs::write(root.join("app.jadpo"), source).unwrap();
        Self(root)
    }
    fn run(&self) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_jadpo"))
            .args(["test", "."])
            .current_dir(&self.0)
            .env("PATH", self.0.join("bin"))
            .env("NO_COLOR", "1")
            .env("JADPO_ASCII", "1")
            .output()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn reject(output: std::process::Output, code: &str) {
    assert!(!output.status.success());
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        text.contains(&jadpo_diagnostics::catalogue_definition(code).rule_id),
        "wanted {code}: {text}"
    );
}
#[test]
fn no_authored_tests_is_a_distinct_error() {
    let root = Fixture::new("type Label = Text {}\n");
    reject(root.run(), "TEST_NO_TESTS");
}
#[test]
fn missing_runtime_is_a_distinct_startup_error() {
    let root = Fixture::new("test \"passing\" { assert true }\n");
    reject(root.run(), "TEST_RUNTIME_START_FAILED");
    assert!(root.0.join("build/target/tests.ts").is_file());
}
#[cfg(unix)]
#[test]
fn unsuccessful_test_process_is_reported_and_success_is_not() {
    use std::os::unix::fs::PermissionsExt;
    let root = Fixture::new("test \"passing\" { assert true }\n");
    let bun = root.0.join("bin/bun");
    fs::write(&bun, "#!/bin/sh\nexit 1\n").unwrap();
    fs::set_permissions(&bun, fs::Permissions::from_mode(0o700)).unwrap();
    reject(root.run(), "TEST_FAILED");
    fs::write(&bun, "#!/bin/sh\nexit 0\n").unwrap();
    assert!(root.run().status.success());
}
