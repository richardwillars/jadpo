//! Independent module contract: docs/grammar-v0.1.md §15, including
//! manifest identity, visibility, explicit imports and acyclic dependencies.
use jadpo_core::{analyze_sources, AnalyzedProject};
use jadpo_diagnostics::Diagnostic;
use jadpo_syntax::SourceFile;
use std::path::PathBuf;

fn analyze(files: &[(&str, &str)]) -> AnalyzedProject {
    analyze_sources(
        files
            .iter()
            .map(|(path, text)| SourceFile::new(PathBuf::from(path), text.to_string()))
            .collect(),
    )
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

fn assert_clean(project: &AnalyzedProject) {
    assert!(
        diagnostics(project).is_empty(),
        "{:#?}",
        diagnostics(project)
    );
}

fn assert_code_at(files: &[(&str, &str)], code: &str, file: &str, excerpt: &str) {
    let project = analyze(files);
    let all = diagnostics(&project);
    let diagnostic = all
        .iter()
        .find(|d| d.code == code)
        .unwrap_or_else(|| panic!("missing {code}: {all:#?}"));
    let span = diagnostic
        .primary
        .as_ref()
        .expect("module diagnostic must locate authored source");
    assert_eq!(span.source, file);
    let source = files.iter().find(|(path, _)| *path == file).unwrap().1;
    assert_eq!(&source[span.start..span.end], excerpt);
    assert!(!diagnostic.message.is_empty());
    assert!(!diagnostic.recommended_next_step.title.is_empty());
}

const DOMAIN: &str =
    "module demo.domain\npublic type DisplayName = Text {}\ntype HiddenName = Text {}";

#[test]
fn imports_are_selective_and_manifest_identity_is_independent_of_directories() {
    let files = [("unexpected/nested/domain.jadpo", DOMAIN), ("api.jadpo", "module demo.api\nimport demo.domain { DisplayName }\npublic type Request = Object { name: DisplayName }")];
    let project = analyze(&files);
    assert_clean(&project);
    let api = project
        .semantics
        .modules
        .iter()
        .find(|m| m.name == "demo.api")
        .unwrap();
    assert_eq!(api.imports, ["demo.domain.DisplayName"]);
    assert_eq!(api.exports, ["Request"]);
    let domain = project
        .semantics
        .modules
        .iter()
        .find(|m| m.name == "demo.domain")
        .unwrap();
    assert_eq!(domain.source, "unexpected/nested/domain.jadpo");
    assert_eq!(domain.exports, ["DisplayName"]);
}

#[test]
fn source_enumeration_order_cannot_change_module_results() {
    let a = ("a.jadpo", DOMAIN);
    let b = ("b.jadpo", "module demo.api\nimport demo.domain { DisplayName }\ntype Request = Object { name: DisplayName }");
    let project = analyze(&[a, b]);
    assert_clean(&project);
    assert_eq!(project, analyze(&[b, a]));
}

#[test]
fn private_unknown_and_missing_imports_have_precise_unicode_adjusted_spans() {
    for (imported, code) in [
        ("HiddenName", "MOD_PRIVATE_IMPORT"),
        ("MissingName", "MOD_UNKNOWN_EXPORT"),
    ] {
        let consumer = format!("// é🦀\nmodule demo.api\nimport demo.domain {{ {imported} }}\ntype Request = Text {{}}");
        assert_code_at(
            &[("domain.jadpo", DOMAIN), ("api.jadpo", &consumer)],
            code,
            "api.jadpo",
            imported,
        );
    }
    assert_code_at(
        &[
            ("domain.jadpo", DOMAIN),
            (
                "api.jadpo",
                "// é🦀\nmodule demo.api\ntype Request = Object { name: DisplayName }",
            ),
        ],
        "MOD_IMPORT_REQUIRED",
        "api.jadpo",
        "DisplayName",
    );
}

#[test]
fn adding_the_required_public_import_repairs_the_failure_without_exporting_private_types() {
    let broken = "module demo.api\ntype Request = Object { name: DisplayName }";
    assert_code_at(
        &[("domain.jadpo", DOMAIN), ("api.jadpo", broken)],
        "MOD_IMPORT_REQUIRED",
        "api.jadpo",
        "DisplayName",
    );
    let repaired = "module demo.api\nimport demo.domain { DisplayName }\ntype Request = Object { name: DisplayName }";
    let project = analyze(&[("domain.jadpo", DOMAIN), ("api.jadpo", repaired)]);
    assert_clean(&project);
    assert_eq!(
        project
            .semantics
            .modules
            .iter()
            .find(|m| m.name == "demo.domain")
            .unwrap()
            .exports,
        ["DisplayName"]
    );
}

#[test]
fn duplicate_imports_are_rejected_within_and_between_import_declarations() {
    for imports in [
        "import demo.domain { DisplayName, DisplayName }",
        "import demo.domain { DisplayName }\nimport demo.domain { DisplayName }",
    ] {
        let consumer = format!("module demo.api\n{imports}\ntype Request = Text {{}}");
        assert_code_at(
            &[("domain.jadpo", DOMAIN), ("api.jadpo", &consumer)],
            "MOD_DUPLICATE_IMPORT",
            "api.jadpo",
            "DisplayName",
        );
        let project = analyze(&[("domain.jadpo", DOMAIN), ("api.jadpo", &consumer)]);
        let duplicate = project
            .semantics
            .diagnostics
            .iter()
            .find(|d| d.code == "MOD_DUPLICATE_IMPORT")
            .unwrap();
        assert_eq!(
            duplicate.primary.as_ref().unwrap().start,
            consumer.rfind("DisplayName").unwrap()
        );
    }
}

#[test]
fn importing_a_local_name_is_a_conflict_and_unknown_modules_are_not_inferred() {
    assert_code_at(
        &[
            ("domain.jadpo", DOMAIN),
            (
                "api.jadpo",
                "module demo.api\nimport demo.domain { DisplayName }\ntype DisplayName = Text {}",
            ),
        ],
        "MOD_IMPORT_CONFLICT",
        "api.jadpo",
        "DisplayName",
    );
    assert_code_at(&[("domain.jadpo", DOMAIN), ("api.jadpo", "module demo.api\nimport unexpected.nested.domain { DisplayName }\ntype Request = Text {}")], "MOD_UNKNOWN_MODULE", "api.jadpo", "import unexpected.nested.domain { DisplayName }");
}

#[test]
fn self_import_and_duplicate_module_identity_are_rejected() {
    assert_code_at(
        &[(
            "api.jadpo",
            "module demo.api\nimport demo.api { DisplayName }\npublic type DisplayName = Text {}",
        )],
        "MOD_SELF_IMPORT",
        "api.jadpo",
        "import demo.api { DisplayName }",
    );
    assert_code_at(
        &[
            ("a.jadpo", "module demo.api\ntype First = Text {}"),
            ("b.jadpo", "module demo.api\ntype Second = Text {}"),
        ],
        "MOD_DUPLICATE_MODULE",
        "b.jadpo",
        "module demo.api",
    );
}

#[test]
fn ambient_compatibility_does_not_leak_into_explicit_modules() {
    let ambient = [
        ("domain.jadpo", "type DisplayName = Text {}"),
        ("api.jadpo", "type Request = Object { name: DisplayName }"),
    ];
    assert_clean(&analyze(&ambient));
    let mixed = analyze(&[("domain.jadpo", DOMAIN), ambient[1]]);
    assert!(diagnostics(&mixed)
        .iter()
        .any(|d| d.code == "MOD_MODULE_REQUIRED"));
}

#[test]
fn longer_import_cycles_are_rejected_but_diamond_dependencies_are_valid() {
    let a = (
        "a.jadpo",
        "module demo.a\nimport demo.b { Bee }\npublic type Aye = Text {}",
    );
    let b = (
        "b.jadpo",
        "module demo.b\nimport demo.c { See }\npublic type Bee = Text {}",
    );
    let c = (
        "c.jadpo",
        "module demo.c\nimport demo.a { Aye }\npublic type See = Text {}",
    );
    let cycle = analyze(&[a, b, c]);
    let cycle_diagnostics = diagnostics(&cycle);
    assert_eq!(
        cycle_diagnostics
            .iter()
            .filter(|d| d.code == "MOD_IMPORT_CYCLE")
            .count(),
        3,
        "{cycle_diagnostics:#?}"
    );
    let diamond = analyze(&[
        ("root.jadpo", "module demo.root\npublic type Root = Text {}"),
        ("left.jadpo", "module demo.left\nimport demo.root { Root }\npublic type Left = Object { value: Root }"),
        ("right.jadpo", "module demo.right\nimport demo.root { Root }\npublic type Right = Object { value: Root }"),
        ("api.jadpo", "module demo.api\nimport demo.left { Left }\nimport demo.right { Right }\ntype Request = Object { left: Left right: Right }"),
    ]);
    assert_clean(&diamond);
}

#[test]
fn imports_are_not_transitive_and_cannot_implicitly_reexport() {
    let middle = ("middle.jadpo", "module demo.middle\nimport demo.domain { DisplayName }\npublic type Envelope = Object { name: DisplayName }");
    let consumer = ("api.jadpo", "module demo.api\nimport demo.middle { Envelope }\ntype Request = Object { name: DisplayName }");
    assert_code_at(
        &[("domain.jadpo", DOMAIN), middle, consumer],
        "MOD_IMPORT_REQUIRED",
        "api.jadpo",
        "DisplayName",
    );
    let reexport = (
        "api.jadpo",
        "module demo.api\nimport demo.middle { DisplayName }\ntype Request = Text {}",
    );
    assert_code_at(
        &[("domain.jadpo", DOMAIN), middle, reexport],
        "MOD_UNKNOWN_EXPORT",
        "api.jadpo",
        "DisplayName",
    );
}

#[test]
fn callable_calls_respect_module_visibility_as_well_as_type_references() {
    let provider = ("provider.jadpo", "module demo.provider\npublic type Greeting = Object { text: Text }\npublic function greeting() -> Greeting { return Greeting { text: \"hello\" } }");
    let consumer = ("api.jadpo", "module demo.api\nimport demo.provider { Greeting }\nfunction response() -> Greeting { return greeting() }");
    assert_code_at(
        &[provider, consumer],
        "MOD_IMPORT_REQUIRED",
        "api.jadpo",
        "greeting",
    );
    assert_clean(&analyze(&[provider, ("api.jadpo", "module demo.api\nimport demo.provider { Greeting, greeting }\nfunction response() -> Greeting { return greeting() }")]));
}
