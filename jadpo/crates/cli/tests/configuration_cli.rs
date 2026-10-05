use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn fixture() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "jadpo-configuration-cli-{}-{nonce}-{}",
        std::process::id(),
        FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).expect("fixture directory");
    fs::write(
        root.join("app.jadpo"),
        r#"type ApiKey = Text { min_length: 3 }
config ApplicationConfiguration {
    api_key: ApiKey { binding: "API_KEY" secret: true }
}
"#,
    )
    .expect("configuration source");
    root
}

#[test]
fn config_check_reports_status_without_values() {
    let root = fixture();
    let executable = env!("CARGO_BIN_EXE_jadpo");

    let missing = Command::new(executable)
        .args(["config", "check"])
        .current_dir(&root)
        .output()
        .expect("missing configuration check");
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr)
        .contains(&jadpo_diagnostics::catalogue_definition("CONFIG_LOCAL_CHECK_FAILED").rule_id));
    assert!(String::from_utf8_lossy(&missing.stdout).contains("api_key: missing"));

    fs::write(root.join(".env.local"), "API_KEY=canary-secret-value\n")
        .expect("local configuration");
    let valid = Command::new(executable)
        .args(["config", "check"])
        .current_dir(&root)
        .output()
        .expect("valid configuration check");
    assert!(
        valid.status.success(),
        "{}",
        String::from_utf8_lossy(&valid.stderr)
    );
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&valid.stdout),
        String::from_utf8_lossy(&valid.stderr)
    );
    assert!(output.contains("api_key: set"));
    assert!(!output.contains("canary-secret-value"));

    fs::remove_dir_all(root).expect("fixture cleanup");
}

#[test]
fn config_set_refuses_a_captured_non_terminal() {
    let root = fixture();
    let output = Command::new(env!("CARGO_BIN_EXE_jadpo"))
        .args(["config", "set", "api_key"])
        .current_dir(&root)
        .output()
        .expect("captured config set");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("interactive terminal"));
    assert!(!root.join(".env.local").exists());
    fs::remove_dir_all(root).expect("fixture cleanup");
}
