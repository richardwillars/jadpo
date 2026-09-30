//! Exercise process failures and default-target preservation through the real CLI.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "jadpo experimental build {} {}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::create_dir(path.join("empty-path")).unwrap();
        Self(path)
    }

    fn checkout(&self, script: &str) {
        for folder in [
            "jadpo/crates/cli",
            "experiments/native-conformance",
            "experiments/wasm-exp1/fixture",
        ] {
            fs::create_dir_all(self.0.join(folder)).unwrap();
        }
        fs::write(self.0.join("jadpo/crates/cli/Cargo.toml"), "").unwrap();
        fs::write(self.0.join("experiments/wasm-exp1/fixture/app.jadpo"), "").unwrap();
        fs::write(
            self.0.join("experiments/native-conformance/build.sh"),
            script,
        )
        .unwrap();
    }

    fn run(&self, args: &[&str], no_tools: bool) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_jadpo"));
        command.args(args).current_dir(&self.0).env("NO_COLOR", "1");
        if no_tools {
            command.env("PATH", self.0.join("empty-path"));
        }
        command.output().unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn reject(output: Output, code: &str) {
    assert!(!output.status.success());
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        text.contains(&jadpo_diagnostics::catalogue_definition(code).rule_id),
        "{text}"
    );
}

#[test]
fn unavailable_checkout_fails_without_starting_tools() {
    let fixture = Fixture::new();
    reject(
        fixture.run(&["build", "--target", "native"], true),
        "CLI_BUILD_EXPERIMENT_UNAVAILABLE",
    );
}

#[test]
fn missing_build_tool_is_a_failure() {
    let fixture = Fixture::new();
    fixture.checkout("exit 0\n");
    reject(
        fixture.run(&["build", "--target", "wasm"], true),
        "CLI_BUILD_TOOL_FAILED",
    );
}

#[cfg(unix)]
#[test]
fn child_failure_propagates_and_success_preserves_target_and_working_directory() {
    let fixture = Fixture::new();
    fixture.checkout("exit 7\n");
    reject(
        fixture.run(&["build", "--target", "native"], false),
        "CLI_BUILD_TOOL_FAILED",
    );
    fixture.checkout(
        "test \"$1\" = --target && test \"$2\" = rust && test -f jadpo/crates/cli/Cargo.toml\n",
    );
    assert!(fixture
        .run(&["build", "--target", "native"], false)
        .status
        .success());
}

#[test]
fn default_and_explicit_bun_builds_generate_the_same_target_without_running_bun() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("app.jadpo"), "type Label = Text {}\n").unwrap();
    assert!(fixture.run(&["build", "."], true).status.success());
    let original = fs::read(fixture.0.join("build/target/app.ts")).unwrap();
    assert!(fixture
        .run(&["build", ".", "--target=bun"], true)
        .status
        .success());
    assert_eq!(
        original,
        fs::read(fixture.0.join("build/target/app.ts")).unwrap()
    );
}
