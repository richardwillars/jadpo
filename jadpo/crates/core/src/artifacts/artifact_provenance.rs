//! Expected fresh generator bytes, not a read of possibly stale written output.
use super::{derive_non_approval_artifacts, AnalyzedProject, GeneratedArtifact};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};

fn pin(outputs: &[GeneratedArtifact], producer: &str) -> Result<BTreeMap<String, Value>, String> {
    let mut files = BTreeMap::new();
    for artifact in outputs {
        if artifact.relative_path.starts_with("approval/") {
            return Err("Approval artifacts must be excluded from their own output pins.".into());
        }
        let bytes = artifact.contents.as_bytes();
        let descriptor = json!({"path":artifact.relative_path,"producer":producer,
            "byte_length":bytes.len(),"byte_digest":format!("sha256:{:x}",Sha256::digest(bytes))});
        if files
            .insert(artifact.relative_path.to_owned(), descriptor)
            .is_some()
        {
            return Err(format!(
                "Generated artifact path appears twice: {}",
                artifact.relative_path
            ));
        }
    }
    Ok(files)
}

/// Caller must supply freshly checked source. Target rejection remains useful
/// inspection evidence, not a successful build or a fabricated empty target.
pub(super) fn derive(path: &Path, project: &AnalyzedProject) -> Result<Value, String> {
    let audits = pin(
        &derive_non_approval_artifacts(path, project),
        "metadata_and_audit",
    )?;
    let (target, target_files) = match crate::derive_target(path, project) {
        Ok(outputs) => (
            json!({"status":"fresh_generator_output","diagnostic":null}),
            pin(&outputs, "bun_target")?,
        ),
        Err(diagnostic) => (
            json!({"status":"generation_rejected","diagnostic":{"code":diagnostic.code,"rule_id":diagnostic.rule_id}}),
            BTreeMap::new(),
        ),
    };
    for name in audits.keys() {
        if target_files.contains_key(name) {
            return Err(format!("Metadata and target output paths overlap: {name}"));
        }
    }
    Ok(
        json!({"status":"fresh_expected_generator_bytes","metadata_and_audit":{"status":"fresh_generator_output","files":audits},
        "target":{"result":target,"files":target_files},"excluded":["approval/subject.json","approval/subject.txt"],
        "written_file_conformance":"not_checked","runtime_conformance":"not_established","release_authority":"not_established"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use jadpo_syntax::SourceFile;

    fn analyzed(text: &str) -> AnalyzedProject {
        let project = crate::analyze_sources(vec![SourceFile::new(
            "/project/app.jadpo".into(),
            text.into(),
        )])
        .unwrap();
        super::super::derive_approval_subject(
            Path::new("/project/app.jadpo"),
            &project,
            None,
            None,
            None,
        )
        .expect("checked fixture");
        project
    }

    #[test]
    fn exact_audit_and_target_bytes_are_pinned_without_approval_recursion() {
        let project=analyzed("output Health { ok: Bool }\naction health() -> Health { return Health { ok: true } }\nroute GET /health { auth: none output: Health run: health() }");
        let path = Path::new("/project/app.jadpo");
        let pins = derive(path, &project).unwrap();
        assert_eq!(pins["target"]["result"]["status"], "fresh_generator_output");
        let mut outputs = derive_non_approval_artifacts(path, &project);
        outputs.extend(crate::derive_target(path, &project).unwrap());
        for output in &outputs {
            let category = if pins["metadata_and_audit"]["files"]
                .get(output.relative_path)
                .is_some()
            {
                "metadata_and_audit"
            } else {
                "target"
            };
            assert_eq!(
                pins[category]["files"][output.relative_path]["byte_digest"],
                format!("sha256:{:x}", Sha256::digest(output.contents.as_bytes()))
            );
            assert_eq!(
                pins[category]["files"][output.relative_path]["byte_length"],
                output.contents.len()
            );
        }
        assert_eq!(
            pins["metadata_and_audit"]["files"]
                .as_object()
                .unwrap()
                .len()
                + pins["target"]["files"].as_object().unwrap().len(),
            outputs.len()
        );
        assert!(pins["metadata_and_audit"]["files"]
            .get("approval/subject.json")
            .is_none());
        assert_eq!(pins["written_file_conformance"], "not_checked");
        assert_eq!(pins, derive(path, &project).unwrap());
        let assembled = super::super::derive_artifacts(path, &project);
        assert_eq!(assembled[8].relative_path, "approval/subject.json");
        assert_eq!(assembled[9].relative_path, "approval/subject.txt");
        assert_eq!(
            assembled
                .iter()
                .filter(|output| !output.relative_path.starts_with("approval/"))
                .map(|output| (output.relative_path, &output.contents))
                .collect::<Vec<_>>(),
            derive_non_approval_artifacts(path, &project)
                .iter()
                .map(|output| (output.relative_path, &output.contents))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn unsupported_job_target_is_rejected_not_an_empty_supported_assertion() {
        let project=analyzed("type ScanTime = Instant {}\naction scan(at: ScanTime) -> Unit {}\njob reminders every 15m { concurrency: singleton run: scan(ScanTime(clock.now)) retry: next_schedule }");
        let pins = derive(Path::new("/project/app.jadpo"), &project).unwrap();
        assert_eq!(pins["target"]["result"]["status"], "generation_rejected");
        assert_eq!(
            pins["target"]["result"]["diagnostic"]["code"],
            "JADPO_TARGET_JOB_NOT_IMPLEMENTED"
        );
        assert!(pins["target"]["files"].as_object().unwrap().is_empty());
        assert!(pins["metadata_and_audit"]["files"]
            .get("audit/jobs.json")
            .is_some());
    }

    #[test]
    fn pins_sort_output_sets_and_reject_duplicate_or_self_referential_paths() {
        let outputs = vec![
            GeneratedArtifact {
                relative_path: "z.txt",
                contents: "one".into(),
            },
            GeneratedArtifact {
                relative_path: "a.txt",
                contents: "two".into(),
            },
        ];
        let mut reordered = outputs.clone();
        reordered.reverse();
        assert_eq!(
            pin(&outputs, "test").unwrap(),
            pin(&reordered, "test").unwrap()
        );
        let mut duplicate = outputs.clone();
        duplicate.push(outputs[0].clone());
        assert!(pin(&duplicate, "test").is_err());
        assert!(pin(
            &[GeneratedArtifact {
                relative_path: "approval/subject.json",
                contents: "{}".into()
            }],
            "test"
        )
        .is_err());
    }

    #[test]
    fn changed_target_bytes_are_visible_and_stale_build_files_are_never_evidence() {
        let text="output Health { ok: Bool }\naction health() -> Health { return Health { ok: true } }\nroute GET /health { auth: none output: Health run: health() }";
        let project = analyzed(text);
        let changed = analyzed(&text.replace("ok: true", "ok: false"));
        let path = Path::new("/project/app.jadpo");
        let before = derive(path, &project).unwrap();
        let after = derive(path, &changed).unwrap();
        assert_ne!(
            before["target"]["files"]["target/app.ts"]["byte_digest"],
            after["target"]["files"]["target/app.ts"]["byte_digest"]
        );
        assert_eq!(
            before["metadata_and_audit"]["files"]["inventory/routes.json"],
            after["metadata_and_audit"]["files"]["inventory/routes.json"]
        );
        // Unique test-owned directory; source is supplied in memory and the
        // decoy output can never be mistaken for a fresh compiler result.
        let root = std::env::temp_dir().join(format!(
            "jadpo-output-pin-{}-{}",
            std::process::id(),
            super::super::OUTPUT_REVISION.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir_all(root.join("build/target")).unwrap();
        std::fs::write(root.join("build/target/app.ts"), "stale malicious decoy").unwrap();
        let input = root.join("app.jadpo");
        let project =
            crate::analyze_sources(vec![SourceFile::new(input.clone(), text.into())]).unwrap();
        let first = derive(&input, &project).unwrap();
        std::fs::write(root.join("build/target/app.ts"), "changed stale decoy").unwrap();
        assert_eq!(first, derive(&input, &project).unwrap());
        assert_eq!(
            first["target"]["files"]["target/app.ts"]["byte_digest"],
            before["target"]["files"]["target/app.ts"]["byte_digest"]
        );
        assert_eq!(
            std::fs::read_to_string(root.join("build/target/app.ts")).unwrap(),
            "changed stale decoy"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn auth_database_and_closed_runtime_dependencies_are_in_the_expected_pin_set() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .unwrap();
        let path = root.join("examples/golden-todo-migration");
        let project = crate::analyze_project(&path).unwrap();
        let pins = derive(&path, &project).unwrap();
        assert_eq!(pins["target"]["result"]["status"], "fresh_generator_output");
        for name in [
            "target/app.ts",
            "target/authentication.ts",
            "target/first-party-authentication.ts",
            "target/jwt-authentication.ts",
            "target/persistence.ts",
            "sql/sqlite/schema.sql",
            "sql/postgres/schema.sql",
            "target/package.json",
            "target/bun.lock",
            "audit/runtime-dependencies.json",
        ] {
            assert!(
                pins["target"]["files"].get(name).is_some(),
                "missing available target/dependency surface {name}"
            );
        }
        assert!(pins["metadata_and_audit"]["files"]
            .get("audit/authentication.json")
            .is_some());
        let target = crate::derive_target(&path, &project).unwrap();
        for output in &target {
            assert_eq!(
                pins["target"]["files"][output.relative_path]["byte_digest"],
                format!("sha256:{:x}", Sha256::digest(output.contents.as_bytes()))
            );
        }
    }
}
