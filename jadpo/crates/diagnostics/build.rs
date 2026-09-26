use std::collections::BTreeSet;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let crates = manifest.parent().expect("crates directory");
    let mut codes = BTreeSet::new();
    visit(crates, &mut codes);
    let mut values = String::new();
    for code in &codes {
        writeln!(values, "    {code:?},").expect("write generated catalogue entry");
    }
    let repository = manifest
        .ancestors()
        .nth(3)
        .expect("diagnostics crate should be inside the repository");
    let fixture_root = repository.join("tests/compile");
    println!("cargo:rerun-if-changed={}", fixture_root.display());
    let mut fixtures = BTreeSet::new();
    visit_fixtures(repository, &fixture_root, &mut fixtures);
    let mut fixture_values = String::new();
    for (code, fixture) in fixtures {
        writeln!(fixture_values, "    ({code:?}, {fixture:?}),")
            .expect("write generated catalogue fixture entry");
    }
    let output = format!(
        "pub const CATALOGUE_CODES: &[&str] = &[\n{values}];\npub const CATALOGUE_FIXTURES: &[(&str, &str)] = &[\n{fixture_values}];\n"
    );
    let destination =
        PathBuf::from(env::var("OUT_DIR").expect("output directory")).join("catalogue_codes.rs");
    fs::write(destination, output).expect("write generated diagnostic catalogue code list");
}

fn visit_fixtures(repository: &Path, path: &Path, fixtures: &mut BTreeSet<(String, String)>) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    let mut entries = entries
        .map(|entry| entry.expect("fixture directory entry").path())
        .collect::<Vec<_>>();
    entries.sort();
    for entry in entries {
        if entry.is_dir() {
            visit_fixtures(repository, &entry, fixtures);
            continue;
        }
        if !entry.to_string_lossy().ends_with(".expect.json") {
            continue;
        }
        println!("cargo:rerun-if-changed={}", entry.display());
        let source = fs::read_to_string(&entry)
            .unwrap_or_else(|error| panic!("read {}: {error}", entry.display()));
        let fixture = entry
            .strip_prefix(repository)
            .expect("fixture should be beneath repository")
            .to_string_lossy()
            .replace('\\', "/");
        for line in source.lines() {
            let Some(marker) = line.find("\"code\"") else {
                continue;
            };
            let remainder = &line[marker + 6..];
            let Some(start) = remainder.find('"') else {
                continue;
            };
            let after_start = &remainder[start + 1..];
            let Some(end) = after_start.find('"') else {
                continue;
            };
            let code = &after_start[..end];
            if is_diagnostic_code(code) {
                fixtures.insert((code.to_owned(), fixture.clone()));
            }
        }
    }
}

fn visit(path: &Path, codes: &mut BTreeSet<String>) {
    let mut entries = fs::read_dir(path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
        .map(|entry| entry.expect("directory entry").path())
        .collect::<Vec<_>>();
    entries.sort();
    for entry in entries {
        if entry.is_dir() {
            if entry.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            visit(&entry, codes);
        } else if entry.extension().is_some_and(|extension| extension == "rs") {
            println!("cargo:rerun-if-changed={}", entry.display());
            let source = fs::read_to_string(&entry)
                .unwrap_or_else(|error| panic!("read {}: {error}", entry.display()));
            for quoted in source.split('"').skip(1).step_by(2) {
                if is_diagnostic_code(quoted) {
                    codes.insert(quoted.to_owned());
                }
            }
        }
    }
}

fn is_diagnostic_code(value: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "CLI_", "DATA_", "EFFECT_", "FAIL_", "FMT_", "INDEX_", "JADPO_", "LSP_", "MIG_", "MOD_",
        "ROUTE_", "RUNTIME_", "SEM_", "SYN_", "TYPE_",
    ];
    value != "JADPO_DEBUG_TARGET_STACKS"
        && value != "JADPO_ASCII"
        && PREFIXES.iter().any(|prefix| {
            value
                .strip_prefix(prefix)
                .is_some_and(|rest| !rest.is_empty())
        })
        && value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}
