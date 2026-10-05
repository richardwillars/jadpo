use jadpo_core::{
    analyze_sources, derive_artifacts, derive_target, format_source, AnalyzedProject,
};
use jadpo_semantic::NodeKind;
use jadpo_syntax::{Declaration, SourceFile};
use std::path::Path;

const SOURCE: &str = r#"type JobRunAt = Instant {}
action scan(at: JobRunAt) -> Unit {}
job overdue_reminders every 15m {
    concurrency: singleton
    run: scan(JobRunAt(clock.now))
    retry: next_schedule
}
"#;

fn analyze(source: &str) -> AnalyzedProject {
    analyze_sources(vec![SourceFile::new("jobs.jadpo".into(), source.into())]).unwrap()
}

fn codes(project: &AnalyzedProject) -> Vec<&str> {
    project
        .syntax
        .diagnostics()
        .chain(project.semantics.diagnostics.iter())
        .chain(project.typing.diagnostics.iter())
        .chain(project.failures.diagnostics.iter())
        .chain(project.policy.diagnostics.iter())
        .map(|diagnostic| diagnostic.code)
        .collect()
}

fn clean(source: &str) -> AnalyzedProject {
    let project = analyze(source);
    assert!(codes(&project).is_empty(), "{:?}", codes(&project));
    project
}

fn rejects(source: &str, expected: &str) {
    let project = analyze(source);
    assert!(
        codes(&project).contains(&expected),
        "expected {expected}, got {:?}\n{source}",
        codes(&project)
    );
}

#[test]
fn typed_job_is_a_nonexecuting_entry_with_intrinsic_clock_and_exact_call_edge() {
    let project = clean(SOURCE);
    let Declaration::Job(job) = &project.syntax.sources[0].file.declarations[2] else {
        panic!("typed job missing")
    };
    assert_eq!(job.every.milliseconds(), Some(900_000));
    assert_eq!(
        &SOURCE[job.range.start..job.range.end],
        SOURCE[SOURCE.find("job ").unwrap()..].trim()
    );
    let node = project.semantics.node("overdue_reminders").unwrap();
    assert_eq!(node.kind, NodeKind::Job);
    let edge = project
        .semantics
        .calls
        .iter()
        .find(|edge| edge.caller == node.id)
        .unwrap();
    assert_eq!(project.semantics.nodes[edge.callee.0 as usize].name, "scan");
    assert_eq!(project.typing.clock_reads.len(), 1);
    assert!(project
        .failures
        .callables
        .iter()
        .all(|entry| entry.callable != "overdue_reminders"));
    assert_eq!(project.failures.jobs[0].callee, "scan");
    assert!(project.failures.jobs[0].failures.is_empty());
    let error = derive_target(Path::new("jobs.jadpo"), &project).unwrap_err();
    assert_eq!(error.code, "JADPO_TARGET_JOB_NOT_IMPLEMENTED");
    assert!(error.reason.contains("no-op"));
}

#[test]
fn job_clauses_are_closed_required_unique_and_not_exportable() {
    for (before, after, expected) in [
        (" every ", " after ", "SYN_JOB_EVERY_REQUIRED"),
        ("15m", "15", "SYN_JOB_DURATION_REQUIRED"),
        (
            "concurrency: singleton",
            "concurrency: parallel",
            "SYN_JOB_CLAUSE_VALUE_INVALID",
        ),
        (
            "retry: next_schedule",
            "retry: forever",
            "SYN_JOB_CLAUSE_VALUE_INVALID",
        ),
        (
            "run: scan(JobRunAt(clock.now))",
            "run: scan",
            "SYN_JOB_RUN_INVOCATION_REQUIRED",
        ),
        (
            "concurrency: singleton",
            "concurrency: singleton concurrency: singleton",
            "SYN_JOB_ITEM_DUPLICATE",
        ),
        (
            "run: scan(JobRunAt(clock.now))",
            "run: scan(JobRunAt(clock.now)) run: scan(JobRunAt(clock.now))",
            "SYN_JOB_ITEM_DUPLICATE",
        ),
        (
            "retry: next_schedule",
            "retry: next_schedule retry: next_schedule",
            "SYN_JOB_ITEM_DUPLICATE",
        ),
        ("retry: next_schedule", "", "SYN_JOB_CLAUSE_REQUIRED"),
        (
            "run: scan(JobRunAt(clock.now))",
            "",
            "SYN_JOB_CLAUSE_REQUIRED",
        ),
        ("concurrency: singleton", "", "SYN_JOB_CLAUSE_REQUIRED"),
        (
            "job overdue_reminders",
            "public job overdue_reminders",
            "SYN_JOB_EXPORT_INVALID",
        ),
    ] {
        rejects(&SOURCE.replace(before, after), expected);
    }
    for field in [
        "principal",
        "identity",
        "version",
        "fence",
        "receipt",
        "outcome",
        "auth",
        "execution",
    ] {
        rejects(
            &SOURCE.replace(
                "retry: next_schedule",
                &format!("{field}: forged retry: next_schedule"),
            ),
            "SYN_JOB_ITEM_UNKNOWN",
        );
    }
    // Nested unknown clauses do not accidentally reinterpret their children.
    rejects(
        &SOURCE.replace(
            "retry: next_schedule",
            "payload: { run: forged(clock.now) } retry: next_schedule",
        ),
        "SYN_JOB_ITEM_UNKNOWN",
    );
}

#[test]
fn job_intervals_are_positive_finite_supported_and_representable() {
    for (value, expected_ms) in [
        ("1ms", 1),
        ("0.001s", 1),
        ("1s", 1_000),
        ("15m", 900_000),
        ("1h", 3_600_000),
        ("0.0000025h", 9),
        ("9007199254740991ms", 9_007_199_254_740_991),
        ("9007199254740.991s", 9_007_199_254_740_991),
        ("1.0000000000000000000000000000000000000000ms", 1),
        ("0.0010000000000000000000000000000000000000s", 1),
    ] {
        let project = clean(&SOURCE.replace("15m", value));
        assert_eq!(project.typing.jobs[0].interval_ms, expected_ms, "{value}");
        let artifact = derive_artifacts(Path::new("jobs.jadpo"), &project)
            .into_iter()
            .find(|artifact| artifact.relative_path == "audit/jobs.json")
            .unwrap();
        let audit: serde_json::Value = serde_json::from_str(&artifact.contents).unwrap();
        assert_eq!(
            audit["jobs"][0]["interval_ms"].as_u64(),
            Some(expected_ms),
            "{value}"
        );
    }
    for value in [
        "0ms",
        "0.0001s",
        "1d",
        "9007199254740992ms",
        "9007199254740991.4ms",
        "0.000999999999999999999999999999999999s",
        "1.00000000000000001ms",
        "0.00000025h",
        "9007199254740.992s",
        "9999999999999999999999999999999999999999h",
    ] {
        let source = SOURCE.replace("15m", value);
        rejects(&source, "TYPE_JOB_INTERVAL_INVALID");
        let project = analyze(&source);
        // Invalid projects cannot enter artifact generation. In particular,
        // no checked interval binding is available to publish a rounded value.
        assert!(project.typing.jobs.is_empty(), "{value}");
    }
    for value in ["-1s", "\"15m\"", "", "cron"] {
        rejects(&SOURCE.replace("15m", value), "SYN_JOB_DURATION_REQUIRED");
    }
}

#[test]
fn jobs_reject_non_action_non_builtin_and_non_closed_run_shapes() {
    for signature in [
        "function scan(at: Instant) -> Unit",
        "query scan(at: Instant) -> Unit",
        "action scan() -> Unit",
        "action scan(at: Instant?) -> Unit",
        "action scan(at: Instant, extra: Instant) -> Unit",
        "action scan(at: Instant) -> Instant",
    ] {
        rejects(
            &SOURCE.replace("action scan(at: JobRunAt) -> Unit", signature),
            "TYPE_JOB_RUN_SIGNATURE_INVALID",
        );
    }
    rejects(
        &format!(
            "type ScheduleTime = Instant {{}}\n{}",
            SOURCE.replace("at: JobRunAt", "at: ScheduleTime")
        ),
        "TYPE_JOB_RUN_ARGUMENT_INVALID",
    );
    rejects(
        &format!(
            "type JobResult = Unit {{}}\n{}",
            SOURCE.replace("-> Unit", "-> JobResult")
        ),
        "TYPE_JOB_RUN_SIGNATURE_INVALID",
    );
    for argument in [
        "",
        "clock.now, clock.now",
        "at: clock.now",
        "principal",
        "Instant(\"2026-01-01T00:00:00Z\")",
        "scan(clock.now)",
    ] {
        rejects(
            &SOURCE.replace(
                "run: scan(JobRunAt(clock.now))",
                &format!("run: scan({argument})"),
            ),
            "TYPE_JOB_RUN_ARGUMENT_INVALID",
        );
    }
    rejects(
        &SOURCE.replace(
            "run: scan(JobRunAt(clock.now))",
            "run: overdue_reminders(clock.now)",
        ),
        "TYPE_JOB_RUN_SIGNATURE_INVALID",
    );
    rejects(
        &SOURCE.replace(
            "run: scan(JobRunAt(clock.now))",
            "run: overdue_reminders(clock.now)",
        ),
        "SEM_NOT_CALLABLE",
    );
    rejects(
        &SOURCE.replace("run: scan(JobRunAt(clock.now))", "run: missing(clock.now)"),
        "SEM_UNKNOWN_CALLEE",
    );
}

#[test]
fn job_names_collisions_and_qualified_callees_keep_existing_namespace_rules() {
    rejects(
        &SOURCE.replace("overdue_reminders", "OverdueReminders"),
        "SEM_NAME_CASE",
    );
    rejects(
        &format!("{SOURCE}\n{}", &SOURCE[SOURCE.find("job ").unwrap()..]),
        "SEM_DUPLICATE_DECLARATION",
    );
    // Known qualified owner succeeds; an invented receiver must not suffix-bind.
    let qualified = SOURCE
        .replace(
            "action scan(at: JobRunAt) -> Unit {}",
            "entity Todo { id: Uuid identity: id action scan(at: JobRunAt) -> Unit {} }",
        )
        .replace("run: scan", "run: Todo.scan");
    clean(&qualified);
    let wrong = qualified.replace("run: Todo.scan", "run: Wrong.scan");
    rejects(&wrong, "SEM_UNKNOWN_CALLEE");
    rejects(&wrong, "TYPE_JOB_RUN_SIGNATURE_INVALID");
    rejects(
        &format!(
            "{SOURCE}\naction invoke(at: Instant) -> Unit {{ var result = overdue_reminders(at) }}"
        ),
        "SEM_NOT_CALLABLE",
    );
}

#[test]
fn job_module_import_visibility_is_checked_without_exporting_schedule_entries() {
    let files = |import: &str| {
        vec![
        SourceFile::new("shared.jadpo".into(), "module shared.jobs\npublic type JobRunAt = Instant {}\npublic action scan(at: JobRunAt) -> Unit {}".into()),
        SourceFile::new("consumer.jadpo".into(), format!("module app.jobs\n{import}\n{}", &SOURCE[SOURCE.find("job ").unwrap()..])),
    ]
    };
    let allowed = analyze_sources(files("import shared.jobs { scan, JobRunAt }")).unwrap();
    assert!(codes(&allowed).is_empty(), "{:?}", codes(&allowed));
    let denied = analyze_sources(files("")).unwrap();
    assert!(codes(&denied).contains(&"MOD_IMPORT_REQUIRED"));
    let missing_constructor_import = analyze_sources(files("import shared.jobs { scan }")).unwrap();
    assert!(codes(&missing_constructor_import).contains(&"MOD_IMPORT_REQUIRED"));
}

#[test]
fn job_nominal_snapshot_rejects_implicit_conversion_and_added_validation() {
    rejects(
        &SOURCE.replace("at: JobRunAt", "at: Instant"),
        "TYPE_PRIMITIVE_SIGNATURE",
    );
    rejects(
        &SOURCE.replace("JobRunAt(clock.now)", "clock.now"),
        "TYPE_JOB_RUN_ARGUMENT_INVALID",
    );
    for base in ["Instant?", "Text", "CalendarDate", "Duration"] {
        rejects(
            &SOURCE.replace("JobRunAt = Instant", &format!("JobRunAt = {base}")),
            "TYPE_JOB_RUN_SIGNATURE_INVALID",
        );
    }
    rejects(
        &SOURCE.replace("Instant {}", "Instant { max_length: 8 }"),
        "TYPE_JOB_RUN_SIGNATURE_INVALID",
    );
    rejects(
        &format!(
            "type ParentTime = Instant {{}}\n{}",
            SOURCE.replace("JobRunAt = Instant", "JobRunAt = ParentTime")
        ),
        "TYPE_JOB_RUN_SIGNATURE_INVALID",
    );
    for expression in [
        "JobRunAt()",
        "JobRunAt(clock.now, clock.now)",
        "JobRunAt(at: clock.now)",
        "JobRunAt(Instant(\"2026-01-01T00:00:00Z\"))",
        "UnknownTime(clock.now)",
    ] {
        rejects(
            &SOURCE.replace("JobRunAt(clock.now)", expression),
            "TYPE_JOB_RUN_ARGUMENT_INVALID",
        );
    }
    rejects(
        &SOURCE.replace("JobRunAt(clock.now)", "UnknownTime(clock.now)"),
        "SEM_UNKNOWN_CALLEE",
    );
}

#[test]
fn job_audit_derives_exact_failure_contract_and_pending_execution_disposition() {
    let source = format!(
        "failure ScanUnavailable {{ kind: Unavailable code: \"scan_unavailable\" }}\n{}",
        SOURCE.replace(
            "-> Unit {}",
            "fails ScanUnavailable -> Unit { reject ScanUnavailable }"
        )
    );
    let project = clean(&source);
    assert_eq!(project.failures.jobs[0].failures, ["ScanUnavailable"]);
    assert!(project.failures.routes.is_empty());
    let audit = derive_artifacts(Path::new("jobs.jadpo"), &project)
        .into_iter()
        .find(|artifact| artifact.relative_path == "audit/jobs.json")
        .unwrap();
    let audit: serde_json::Value = serde_json::from_str(&audit.contents).unwrap();
    assert_eq!(
        audit["jobs"][0]["failures"],
        serde_json::json!(["ScanUnavailable"])
    );
    assert_eq!(audit["jobs"][0]["runtime_lowering_supported"], false);
    assert_eq!(
        audit["jobs"][0]["durable_failure_dispositions"],
        "pending_checked_worker_binding"
    );
    assert_eq!(
        audit["jobs"][0]["execution_profile"],
        serde_json::Value::Null
    );
    assert_eq!(audit["jobs"][0]["argument"]["constructor"], "JobRunAt");
    assert_eq!(
        audit["jobs"][0]["constructor_validation_proof"],
        "trusted_intrinsic_instant_to_unconstrained_direct_nominal_instant"
    );
}

#[test]
fn job_audit_and_approval_retain_transitive_service_effects_without_execution_claim() {
    let fixture =
        include_str!("../../../../tests/compile/pass/170_checked_service_operation.jadpo");
    let source = format!(
        r#"{fixture}
type JobRunAt = Instant {{}}
failure JobIdentityMissing {{ kind: NotFound code: "job_identity_missing" }}
query find_job_identity()
    freshness: authoritative
    fails JobIdentityMissing
    -> ServiceFakeIdentity
{{
    return attempt query required ServiceFakeIdentity {{
        where: label == ServiceFakeIdentity.label("fixture-only")
        missing: JobIdentityMissing
    }}
}}
action scan(at: JobRunAt)
    fails JobIdentityMissing, ReminderRecipientRejected, ReminderTemporarilyUnavailable, Misconfigured, Unavailable, OutcomeUnknown
    -> Unit
{{
    var source_record = attempt find_job_identity()
    var mail = ReminderMessage {{
        idempotency_key: ReminderMessage.idempotency_key(source_record.id)
        from: ReminderMessage.from("sender@example.test")
        to: ReminderMessage.to("recipient@example.test")
        todo_title: ReminderMessage.todo_title("Job audit only")
        due_at: none
    }}
    var receipt = attempt deliver(mail)
}}
job overdue_reminders every 15m {{
    concurrency: singleton
    run: scan(JobRunAt(clock.now))
    retry: next_schedule
}}
"#
    );
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let project = analyze_sources(vec![SourceFile::new(
        repository.join("job-audit.jadpo"),
        source,
    )])
    .unwrap();
    assert!(codes(&project).is_empty(), "{:?}", codes(&project));
    assert!(project.failures.jobs[0].may_suspend);
    assert!(project.failures.jobs[0]
        .failures
        .contains(&"OutcomeUnknown".to_owned()));
    let artifacts = derive_artifacts(&repository.join("job-audit.jadpo"), &project);
    let audit: serde_json::Value = serde_json::from_str(
        &artifacts
            .iter()
            .find(|value| value.relative_path == "audit/jobs.json")
            .unwrap()
            .contents,
    )
    .unwrap();
    assert_eq!(
        audit["jobs"][0]["external_service_effects"],
        serde_json::json!(["ReminderMail.send_overdue_reminder"])
    );
    assert_eq!(audit["jobs"][0]["runtime_lowering_supported"], false);
    let approval: serde_json::Value = serde_json::from_str(
        &artifacts
            .iter()
            .find(|value| value.relative_path == "approval/subject.json")
            .unwrap()
            .contents,
    )
    .unwrap();
    assert!(approval
        .to_string()
        .contains("reachability:overdue_reminders"));
    assert!(derive_target(&repository.join("job-audit.jadpo"), &project).is_err());
}

#[test]
fn job_formatter_is_idempotent_and_preserves_typed_meaning() {
    let source = SOURCE
        .replace("    ", "\t  ")
        .replace("run: scan", "run :  scan");
    let formatted = format_source(&source);
    assert_eq!(formatted, SOURCE);
    assert_eq!(format_source(&formatted), formatted);
    let before = clean(&source);
    let after = clean(&formatted);
    assert_eq!(before.failures.jobs, after.failures.jobs);
    assert_eq!(before.semantics.calls, after.semantics.calls);
}
